use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use axum_extra::headers::{ETag, Header, HeaderMapExt, IfNoneMatch};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

pub async fn layer(req: Request, next: Next) -> Response {
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

#[inline]
pub fn weaken(etag: ETag) -> ETag {
    let mut header = vec![];
    etag.encode(&mut header);
    format!(
        "\"W/{tag}",
        tag = &header[0].to_str().unwrap_or_default()[1..]
    )
    .parse()
    .unwrap_or(etag)
}

pub mod revid {
    use axum::{
        extract::{Query, Request},
        http::StatusCode,
        middleware::Next,
        response::{IntoResponse, Response},
        RequestExt,
    };
    use axum_extra::headers::{ETag, HeaderMapExt, IfNoneMatch};
    use serde::Deserialize;
    use std::{
        fs::File,
        io::{self, Read},
        path::Path,
        str,
    };

    #[derive(Debug, Deserialize)]
    struct Param {
        revid: Option<String>,
    }

    impl Param {
        fn into_header(self) -> Result<Option<IfNoneMatch>, StatusCode> {
            self.revid
                .map(|id| {
                    format!("\"{id}\"")
                        .parse::<ETag>()
                        .map(IfNoneMatch::from)
                        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)
                })
                .transpose()
        }
    }

    pub async fn converter(mut req: Request, next: Next) -> Result<Response, Response>
where {
        let header = req
            .extract_parts::<Query<Param>>()
            .await
            .map_err(|e| e.into_response())
            .and_then(|Query(param)| param.into_header().map_err(|e| e.into_response()))?;

        if let Some(header) = header {
            tracing::info!("converted revid to {header:?}");
            req.headers_mut().typed_insert(header);
        }

        Ok(next.run(req).await)
    }

    pub fn from_ldml(path: &Path) -> Option<ETag> {
        // Only grab the first 4K of any ldml file as we expect to find the
        // <sil:identity> tag in that region.
        let mut buf = [0; 1 << 12];
        let head = File::open(path)
            .and_then(|mut file| file.read(&mut buf))
            .and_then(|len| {
                str::from_utf8(&buf[..len])
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
            })
            .ok()?;

        // Search for the revid= attribute.
        let token = head.find("revid=\"").and_then(|start| {
            let start = start + "revid=".len();
            head[start + 1..]
                .find('"')
                .map(|end| &head[start..=start + end + 1])
        })?;

        token.parse::<ETag>().ok()
    }
}
