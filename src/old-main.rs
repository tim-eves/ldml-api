#![feature(
    binary_heap_drain_sorted,
    decl_macro, 
    option_result_contains,
    proc_macro_hygiene, 
    trace_macros,
    try_trait,
)]
use rocket::{
    Request,
    Response,
    State,
    config::{ Config, ConfigError, Value },
    fairing::{ AdHoc },
    http::{ RawStr, Status, hyper::header },
    request::{ FromFormValue, FromParam },
    response::{ self, NamedFile, Responder, Stream, status }
};
use std::{
    collections::HashSet,
    fs, 
    io,
    ops::Deref,
    path::{ Path, PathBuf },
    str::FromStr,
    sync::Arc,
};

// #[macro_use] extern crate rocket;

// mod tag;
// mod langtags;

// /*
// /<ws_id>                => /<ws_id> [Accept:application/x.vnd.sil.ldml.v2+xml]
//     [ext=<type>]        => [Accept: application/vnd.sil.ldml.v2+<type>...]
//     [flatten=<bool>]    => [flatten=<bool>]
//     [inc=<top>[,..]]    => [inc=<top>[,..]]
//     [revid=<etag>]      => [If-Not-Match: <etag>][Accept: application/vnd.sil.ldml.v2+<type>...]
//     [uid=<uuid>]        => [uid=<uuid>]
//     [staging=<bool>]    => [Accept: application/vnd.sil.ldml.v2+<type>+staging,...]
// /?query=langtags[&ext=<type>]           => /langtags [Accept: application/vnd.sil.ldml.v2+<type>...]
// /<ws_id>?query=tags[&ext=<type>]        => /tagset/<ws_id> [Accept: application/vnd.sil.ldml.v2+txt]
// /?ws_id=<ws_id>                         => /<ws_id> [Accept:application/x.vnd.sil.ldml.v2+xml]
// */

// use tag::Tag;
// use langtags::{ LangTags, TagSet };

// impl<'a> FromParam<'a> for Tag {
//     type Error = &'a RawStr;

//     #[inline(always)]
//     fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
//         Tag::from_str(param.as_str()).map_err(|_| param)
//     }
// }

// impl<'a> FromFormValue<'a> for Tag {
//     type Error = &'a RawStr;

//     #[inline(always)]
//     fn from_form_value(param: &'a RawStr) -> Result<Self, Self::Error> {
//         Tag::from_str(param.as_str()).map_err(|_| param)
//     }
// }

// #[derive(Debug, Clone, Copy, Default)]
// struct Toggle(bool);

// impl Toggle {
//     const ON: Toggle = Toggle(true);
//     const OFF: Toggle = Toggle(false);
// }

// impl Deref for Toggle {
//     type Target = bool;
//     fn deref(&self) -> &Self::Target { &self.0 }
// }

// impl<'a> FromFormValue<'a> for Toggle {
//     type Error = &'a RawStr;

//     #[inline(always)]
//     fn from_form_value(param: &'a RawStr) -> Result<Self, Self::Error> {
//         Ok(match param.to_ascii_lowercase().as_str() {
//             ""|"0"|"no"|"false"|"off" => Toggle::OFF, 
//             _ => Toggle::ON
//         })    
//     }    
// }    


#[derive(Debug)]
struct SendFile(NamedFile);

impl<'r> Responder<'r> for SendFile {
    fn respond_to(self, rq: &Request) -> response::Result<'r> {
        let cfg = rq.guard::<State<APIConfig>>().unwrap();
        let path = self.0.path().to_owned();
        let mut rsp = self.0.respond_to(rq)?;

        if let Some(method) = &cfg.sendfile_method {
            let disposition = rq.headers().get_one("Content-Disposition")
                .map(str::to_string)
                .unwrap_or({
                    let filename = path.file_name().unwrap();
                    format!("attachment; filename={:?}", filename.to_string_lossy())
                });
            rsp.take_body();
            
            Response::build_from(rsp)
                .raw_header(method.clone(), path.to_string_lossy().into_owned())
                .raw_header("Content-Disposition", disposition)
                .ok()
        } else {
            Ok(rsp)
        }
    }
}

struct ETag<R>(R, Option<header::EntityTag>);

impl<'r, R> Responder<'r> for ETag<R> 
where
    R: Responder<'r>
{
    fn respond_to(self, rq: &Request) -> response::Result<'r> {
        let incoming: Option<_> = rq.get_query_value::<String>("revid")
            .and_then(Result::ok)
            .map(|v| format!("\"{}\"", v))
            .or(rq.headers().get_one("If-None-Match")
                .map(|s| s.to_string()));

        let mut rsp = self.0.respond_to(rq)?;

        if let Some(target) = incoming {
            let etag = str::parse::<header::EntityTag>(&target)
            .map_err(|_| Status::BadRequest)?;

            if Some(etag) == self.1 {
                return Response::build().status(Status::NotModified).ok();
            }
        }
        if let Some(etag) = self.1 { 
            rsp.set_header(header::ETag(etag)); 
        }
        Ok(rsp)
    }
}

// #[get("/?query=alltags", rank=0)]
// fn query_alltags() -> status::NotFound<&'static str> {
//     status::NotFound("LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'.")
// }

// #[get("/<_ws_id>?query=alltags", rank=3)]
// fn ldml_query_alltags(_ws_id: &RawStr) -> status::NotFound<&'static str> {
//     query_alltags()
// }

// #[get("/?query=langtags&<ext>&<staging>")]
// fn query_langtags(ext: Option<String>, staging: Option<Toggle>, cfg: State<APIConfig>) -> Option<SendFile> {
//     let ext = ext.as_deref().unwrap_or("txt");
//     langtags("langtags.".to_string() + ext, staging, cfg)
// }    

// #[get("/<_ws_id>?query=langtags&<ext>&<staging>")]
// fn ldml_query_langtags(_ws_id: &RawStr, ext: Option<String>, staging: Option<Toggle>, cfg: State<APIConfig>) -> Option<SendFile> {
//     query_langtags(ext, staging, cfg)
// }

#[get("/<langtags>?<staging>", rank=2)]
fn langtags(langtags: String, staging: Option<Toggle>, cfg: State<APIConfig>) -> Option<SendFile> {
    let path = cfg.langtags_path(*staging.unwrap_or_default());
    match langtags.as_str() {
        "langtags.txt"|"langtags.json" => NamedFile::open(path.join(langtags))
                                            .map(SendFile).ok(),
        _ => None                                    
    }    
}    

#[get("/<ws_id>?query=tags&<staging>", rank=0)]
fn query_tags(ws_id: Tag, staging: Option<Toggle>, cfg: State<APIConfig>) -> Option<String> {
    use std::collections::BinaryHeap;
    let sets = cfg.langtags(*staging.unwrap_or_default()).iter()
        .fold(HashSet::<*const TagSet>::new(), |mut s, (k, tagset)| {
            if ws_id.lang == k.lang && ws_id.region == k.region { 
                s.insert(Arc::as_ptr(tagset));
            }
            s
    });
    if sets.is_empty() {
        return None;
    }
    Some(sets.into_iter().collect::<BinaryHeap<_>>().drain_sorted()
            .fold(String::new(), |s, t| { 
                let tag = unsafe { t.as_ref().unwrap() };
                s + &tag.to_string() + "\n" 
            })
    )
}

#[get("/?<ws_id>&<ext>&<flatten>&<inc>&<revid>&<staging>&<uid>", rank=0)]
fn ldml_query_ws(ws_id: Tag,
                 ext: Option<String>,
                 flatten: Option<Toggle>,
                 inc: Option<String>,
                 revid: Option<&RawStr>,
                 staging: Option<Toggle>,
                 uid: Option<u32>,
                 cfg: State<APIConfig>) -> Result<LDML, status::NotFound<String>> 
{
    ldml(ws_id, ext, flatten, inc, revid, staging, uid, cfg)
}

#[derive(Responder)]
enum LDML {
    Static(ETag<SendFile>),
    Dynamic(ETag<Stream<ChannelReader>>)
}

#[get("/<ws_id>?<ext>&<flatten>&<inc>&<revid>&<staging>&<uid>", rank=1)]
fn ldml(ws_id: Tag,
        ext: Option<String>,
        flatten: Option<Toggle>,
        inc: Option<String>,
        revid: Option<&RawStr>,
        staging: Option<Toggle>,
        uid: Option<u32>,
        cfg: State<APIConfig>) -> Result<LDML, status::NotFound<String>>
{    
    let sldr_dir = cfg.sldr_path(
        *staging.unwrap_or_default(), 
        *flatten.unwrap_or(Toggle::ON));
    let langtags = cfg.langtags(*staging.unwrap_or_default());
    let _ext = ext.unwrap_or("xml".into());
    let not_found_status = || status::NotFound(format!("No LDML for {}\n", ws_id));

    let ldml_path = find_ldml_file(&ws_id, &sldr_dir, &langtags)
        .ok_or_else(not_found_status)?;
    let etag = revid
        .and(fs::File::open(&ldml_path).ok())
        .and_then(get_revid_from_ldml)
        .map(if inc.is_some() { header::EntityTag::weak } 
             else             { header::EntityTag::strong });
    if let Some(inc) = inc {
        let toplevels: Vec<&str> = inc.split(",").map(str::trim).collect();
        let filtered = filter_toplevels_from_ldml(
            fs::File::open(ldml_path).map_err(|_| not_found_status())?, 
            &toplevels);
        Ok(LDML::Dynamic(ETag(Stream::chunked(filtered, 1<<12), etag)))
    } else {
        Ok(LDML::Static(ETag(
            SendFile(NamedFile::open(ldml_path)
                .map_err(|_| not_found_status())?), 
            etag)))
    }
}


// #[get("/", rank=10)]
// fn doc() -> NamedFile
// {
//     NamedFile::open("static/index.html").unwrap()
// }    

fn find_ldml_file(
        ws_id: &Tag, 
        sldr_dir: &Path, langtags: 
        &LangTags) -> Option<PathBuf> {
    // Lookup the tag set and generate a prefered sorted list.
    let mut tagset: Vec<_> = langtags.get(&ws_id)?.iter().collect();
    tagset.sort_by(|a, b| a.partial_cmp(b).unwrap());
    tagset.push(&ws_id);
    tagset.iter()
        .map(|&tag| {
            let mut path = PathBuf::from(sldr_dir);
            path.push(&tag.lang[0..1]);
            path.push(tag.to_string().replace("-","_"));
            path.with_extension("xml")
        }).rfind(|path| path.exists())
}

fn get_revid_from_ldml<R: io::Read>(reader: R) -> Option<String> {
    use xml::reader::{ EventReader, XmlEvent };

    let sil_identity = EventReader::new(reader).into_iter()
        .find(|event| {
            if let Ok(XmlEvent::StartElement{name,..}) = event {
                name.prefix.as_deref().unwrap_or("") == "sil"
                && name.local_name == "identity" 
            } else { false } 
    }); 
    if let Some(Ok(XmlEvent::StartElement{attributes,..})) = sil_identity
    {
        attributes.iter()
            .find(|attr| attr.name.local_name == "revid")
            .map(|attr| attr.value.clone())
    } else { None }
}

fn filter_toplevels_from_ldml<R1:'static + io::Read + Send + Sync>(reader: R1, toplevels: &[&str]) -> ChannelReader {
    use std::collections::BTreeSet;
    use xml::reader::XmlEvent;

    let (sender, receiver) = bytes_channel();
    let mut include: BTreeSet<String>  = toplevels.iter().map(|s| s.to_string()).collect();
    include.insert("ldml".to_owned());
    include.insert("identity".to_owned());

    std::thread::spawn(move || {
        let reader = xml::ParserConfig::new()
            .trim_whitespace(true)
            .create_reader(io::BufReader::new(reader));
        let mut writer = xml::EmitterConfig::new()
            .write_document_declaration(false)
            .keep_element_names_stack(false)
            .perform_indent(true)
            .indent_string("\t")
            .create_writer(io::BufWriter::new(sender));

        let mut skip = false;
        let mut level = 0;
        for e in reader {
            let e = e.unwrap();
            match e {
                XmlEvent::StartElement{ref name, ..} => {
                    let tag = name.to_string();
                    if level == 1 && !include.contains(&tag) { skip = true; }
                    level += 1;
                },
                XmlEvent::EndElement{..} => {
                    level -= 1;
                    if level == 1 && skip { skip = false; continue; }
                },
                _ => ()
            }
            if !skip {
                e.as_writer_event().map(|e| writer.write(e));
            }
        }
    });
    receiver
}

// #[derive(Debug)]
// struct APIConfig {
//     sldr_dirs: (PathBuf, PathBuf),
//     langtags_dirs: (PathBuf, PathBuf),
//     langtags: (LangTags, LangTags),
//     sendfile_method: Option<String>
// }

// impl APIConfig {
//     fn langtags(&self, staging: bool) -> &LangTags { 
//         if staging { &self.langtags.1 } else { &self.langtags.0 }
//     }

//     fn langtags_path(&self, staging: bool) -> &Path { 
//         if staging { &self.langtags_dirs.1 } else { &self.langtags_dirs.0 }
//     }

//     fn sldr_path(&self, staging: bool, flat: bool) -> PathBuf {
//         if staging { &self.sldr_dirs.1 } else { &self.sldr_dirs.0 }
//             .join(if flat { "flat" } else { "unflat" })
//     }

//     fn get(config: &Config) -> Result<APIConfig, ConfigError> {
//         use std::fs::File;

//         let sendfile = config.get_str("sendfile_method").map(str::to_string).ok();
//         config.get_table("ldml")
//             .and_then(|ldml| {
//                 let staging = ldml.get("staging").and_then(Value::as_table);
//                 let sp: PathBuf = ldml.get("sldr")
//                     .and_then(Value::as_str)
//                     .unwrap_or("static/")
//                     .into();
//                 let lp: PathBuf = ldml.get("langtags")
//                     .and_then(Value::as_str)
//                     .unwrap_or("static/")
//                     .into();
//                 let ss: PathBuf = staging
//                     .and_then(|t| t.get("sldr")
//                         .and_then(Value::as_str)
//                         .map(PathBuf::from))
//                     .unwrap_or(sp.clone());
//                 let ls: PathBuf = staging
//                     .and_then(|t| t.get("langtags")
//                         .and_then(Value::as_str)
//                         .map(PathBuf::from))
//                     .unwrap_or(lp.clone());
//                 let langtags = File::open(lp.join("langtags.txt"))
//                     .and_then(LangTags::from_reader)
//                     .map_err(|err| ConfigError::Io(err, "ldml.langtags"))?;
//                 let langtags_staging = File::open(ls.join("langtags.txt"))
//                     .and_then(LangTags::from_reader)
//                     .map_err(|err| ConfigError::Io(err, "ldml.staging.langtags"))?;
//                 Ok(APIConfig {
//                     sldr_dirs:       (sp, ss),
//                     langtags_dirs:   (lp, ls),
//                     langtags:        (langtags, langtags_staging),
//                     sendfile_method: sendfile
//                 })
//             })
//     }
// }

// fn main() -> Result<(), std::io::Error> {
//     rocket::ignite()
//         .mount("/", routes![
//             doc,
//             query_alltags, ldml_query_alltags,
//             langtags, query_langtags, ldml_query_langtags,
//             query_tags,
//             ldml])
//         .attach(AdHoc::on_attach("SLDR Config", |rocket| {
//             if let Ok(ldml) = APIConfig::get(rocket.config()) {
//                 Ok(rocket.manage(ldml))
//             } else {
//                 Err(rocket)
//             }
//         }))
//         .launch();
//     Ok(())
// }

use std::{
    io::{Read, Write},
    sync::mpsc::{SyncSender, RecvError, Receiver, sync_channel }
};


struct ChannelReader(Receiver<Box<[u8]>>, io::Cursor<Box<[u8]>>);
impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut len = 0;
        while len < buf.len() {
            match self.1.read(&mut buf[len..])? {
                0 => match self.0.recv() {
                    Ok(msg) => self.1 = io::Cursor::new(msg),
                    Err(RecvError) => break
                },
                n => len += n
            }
        };
        Ok(len)
    }
}

struct ChannelWriter(SyncSender<Box<[u8]>>);
impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf.to_owned().into_boxed_slice())
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

fn bytes_channel() -> (ChannelWriter, ChannelReader) {
    let (sender, recevier) = sync_channel(1);
    (ChannelWriter(sender),
     ChannelReader(recevier, io::Cursor::new(vec![].into_boxed_slice())))
}
