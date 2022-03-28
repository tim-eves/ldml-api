use axum::{
    headers::{ETag, HeaderMapExt, IfNoneMatch},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{fs, hash::{Hash, Hasher}, path::Path};

pub async fn layer<B>(req: Request<B>, next: Next<B>) -> Response
where
    B: Send,
{
    let header = req.headers().typed_get::<IfNoneMatch>();
    let mut rsp = next.run(req).await;
    let etag = rsp.headers().typed_get::<ETag>();
    if let Some(if_none_match) = header {
        tracing::info!("Precondition: {if_none_match:?}");
        if let Some(etag) = etag {
            tracing::info!("Response etag: {etag:?}");
            if !if_none_match.precondition_passes(&etag) {
                *rsp.status_mut() = StatusCode::NOT_MODIFIED;
                tracing::info!("IfNoneMatch precondition fails, ETag matched");
            }
        }
    }
    rsp
}

pub fn from_metadata(path: &Path) -> Option<ETag> {
    let meta = fs::metadata(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    meta.modified().ok()?.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    let token = format!("\"{hash:x}\"", hash = hasher.finish());
    token.parse::<ETag>().ok()
}

pub mod revid {
    use axum::{
        extract::{FromRequest, Query, RequestParts},
        headers::{ETag, HeaderMapExt, IfNoneMatch},
        http::{Request, StatusCode},
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use serde::Deserialize;
    use std::{fs::File, io::{self, Read}, path::Path, str};

    #[derive(Debug, Deserialize)]
    struct Param {
        revid: Option<String>,
    }

    impl Param {
        fn into_header(&self) -> Result<Option<IfNoneMatch>, StatusCode> {
            self.revid
                .as_ref()
                .map(|id| {
                    format!("\"{id}\"")
                        .parse::<ETag>()
                        .map(IfNoneMatch::from)
                        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)
                })
                .transpose()
        }
    }

    pub async fn converter<B>(req: Request<B>, next: Next<B>) -> Result<Response, Response>
    where
        B: Send,
    {
        let mut parts = RequestParts::new(req);
        let header = Query::<Param>::from_request(&mut parts)
            .await
            .map_err(|e| e.into_response())
            .and_then(|Query(param)| param.into_header().map_err(|e| e.into_response()))?;
        if let Some(header) = header {
            tracing::info!("converted revid to {header:?}");
            parts
                .headers_mut()
                .expect("Headers already extracted")
                .typed_insert(header);
        }

        Ok(next
            .run(
                parts
                    .try_into_request()
                    .expect("failed to assemble request"),
            )
            .await)
    }

    pub fn from_ldml(path: &Path) -> Option<ETag> {
        // Only grab the first 4K of any ldml file as we expect to find the
        // <sil:identity> tag in that region.
        let mut buf = [0; 1 << 12];
        let head = File::open(path)
            .and_then(|mut res| res.read(&mut buf))
            .map(|len| &buf[..len])
            .and_then(|buf| {
                str::from_utf8(&buf).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
            })
            .ok()?;
    
        // Search for the revid= attribute.
        let token = head.find("revid=\"").and_then(|start| {
            let start = start + "revid=".len();
            head[start+1..].find('"').map(|end| &head[start..=start+end+1])
        })?;
    
        token.parse::<ETag>().ok()
    }    
}
