use core::fmt;
use libxml::{
    parser::{Parser, ParserOptions},
    readonly::RoNode,
    tree::{self, document::SaveOptions},
    xpath,
};
use std::{io, path::Path};

pub struct Document {
    inner: tree::Document,
}

impl Document {
    pub fn new<'a>(path: impl AsRef<Path> + 'a) -> io::Result<Self> {
        // let doc = fs::read(path)?;
        let parser: Parser = Default::default();
        let inner = parser
            .parse_file_with_options(
                path.as_ref().to_str().ok_or(io::ErrorKind::InvalidInput)?,
                ParserOptions {
                    no_def_dtd: true,
                    no_blanks: true,
                    no_net: true,
                    no_implied: true,
                    compact: true,
                    ..Default::default()
                },
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Document { inner })
    }

    fn get_context(&self) -> Option<xpath::Context> {
        let ctxt = xpath::Context::new(&self.inner).ok()?;
        ctxt.register_namespace("sil", "urn://www.sil.org/ldml/0.1")
            .ok()?;
        Some(ctxt)
    }

    pub fn find_nodes(&self, xpath: &str) -> Option<Vec<RoNode>> {
        let root = self.inner.get_root_readonly()?;
        let ctxt = self.get_context()?;
        ctxt.node_evaluate_readonly(xpath, root)
            .ok()
            .map(|res| res.get_readonly_nodes_as_vec())
    }

    pub fn _find_value(&self, xpath: &str) -> Option<String> {
        self.get_context()
            .and_then(|mut ctxt| ctxt.findvalue(xpath, None).ok())
    }

    pub fn subset(&mut self, xpaths: &[&str]) -> Result<(), String> {
        let compound =
            "/ldml/*[self::".to_string() + &xpaths.join(" or self::") + " or self::identity]";
        let nodes = self.find_nodes(&compound).ok_or("XPath evalution failed")?;
        let ldml = self
            .inner
            .get_root_element()
            .ok_or("Malformed LDML document")?;
        let mut toplevel = ldml.get_first_element_child();
        while let Some(mut node) = toplevel {
            toplevel = node.get_next_element_sibling();
            if nodes.iter().all(|i| i.to_hashable() != node.to_hashable()) {
                node.unlink();
            }
        }
        Ok(())
    }

    pub fn set_uid(&mut self, uid: u32) -> Result<(), String> {
        let mut ctxt = self.get_context().ok_or("XPath context creation failed")?;
        let mut nodes = ctxt
            .findnodes("//sil:identity", None)
            .or(Err("XPath evalution failed"))?;
        let silident = nodes.first_mut().ok_or("sil::identity node not found")?;
        silident
            .set_attribute("uid", &uid.to_string())
            .map_err(|err| format!("Failed to set uid attribute: {err}"))?;
        Ok(())
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner.to_string_with_options(SaveOptions {
            format: true,
            no_empty_tags: false,
            no_xhtml: true,
            non_significant_whitespace: true,
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod test {
    use super::Document;

    #[test]
    fn parse_ldml_doc() {
        let doc = Document::new("tests/en_US.xml");

        assert!(doc.is_ok());
    }

    #[test]
    fn find_revid() {
        let doc = Document::new("tests/en_US.xml").expect("should parse test LDML");
        let revid = doc
            ._find_value("//sil:identity/@revid")
            .expect("should find revid");

        assert_eq!(revid, "b83dea0b8c92193966b10b85c823a22479d1c3ed");
    }

    #[test]
    fn update_uid() {
        let mut doc = Document::new("tests/en_US.xml").expect("should parse test LDML");
        doc.set_uid(012345678).expect("should update uid");
        let uid = doc
            ._find_value("//sil:identity/@uid")
            .expect("should find uid attribute");
        assert_eq!(uid, "12345678");
    }

    #[test]
    fn find_sil_kdb() {
        let doc = Document::new("tests/en_US.xml").expect("should parse test LDML");
        let silkbd = doc
            ._find_value("//sil:kbd[@id='basic_kbdusa']/sil:url")
            .expect("should find `basic_kbdusa` url");

        assert_eq!(
            silkbd,
            "https://keyman.com/go/keyboard/basic_kbdusa/download/kmp"
        );
    }

    #[test]
    fn subsetting() {
        let mut doc = Document::new("tests/en_US.xml").expect("should parse test LDML");
        doc.subset(&["metadata", "layout"])
            .expect("should subset test LDML Document");

        assert_eq!(
            doc.to_string(),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Copyright © 1991-2019 Unicode, Inc.
For terms of use, see http://www.unicode.org/copyright.html
Unicode and the Unicode Logo are registered trademarks of Unicode, Inc. in the U.S. and other countries.
CLDR data files are interpreted according to the LDML specification (http://unicode.org/reports/tr35/)
-->
<ldml>
  <identity>
    <version number="$Revision$"/>
    <language type="en"/>
    <territory type="US"/>
    <special xmlns:sil="urn://www.sil.org/ldml/0.1">
      <sil:identity defaultRegion="US" revid="b83dea0b8c92193966b10b85c823a22479d1c3ed" script="Latn" source="cldr"/>
    </special>
  </identity>
  <layout>
    <orientation>
      <characterOrder>left-to-right</characterOrder>
      <lineOrder>top-to-bottom</lineOrder>
    </orientation>
  </layout>
  <metadata>
    <casingData>
      <casingItem type="calendar_field">lowercase</casingItem>
      <casingItem type="currencyName">titlecase</casingItem>
      <casingItem type="currencyName_count">titlecase</casingItem>
      <casingItem type="day_format_except_narrow">titlecase</casingItem>
      <casingItem type="day_narrow">titlecase</casingItem>
      <casingItem type="day_standalone_except_narrow">titlecase</casingItem>
      <casingItem type="era_abbr">titlecase</casingItem>
      <casingItem type="era_name">titlecase</casingItem>
      <casingItem type="era_narrow">titlecase</casingItem>
      <casingItem type="key">titlecase</casingItem>
      <casingItem type="keyValue">titlecase</casingItem>
      <casingItem type="language">titlecase</casingItem>
      <casingItem type="metazone_long">titlecase</casingItem>
      <casingItem type="month_format_except_narrow">titlecase</casingItem>
      <casingItem type="month_narrow">titlecase</casingItem>
      <casingItem type="month_standalone_except_narrow">titlecase</casingItem>
      <casingItem type="quarter_abbreviated">titlecase</casingItem>
      <casingItem type="quarter_format_wide">titlecase</casingItem>
      <casingItem type="quarter_standalone_wide">titlecase</casingItem>
      <casingItem type="relative">lowercase</casingItem>
      <casingItem type="script">titlecase</casingItem>
      <casingItem type="territory">titlecase</casingItem>
      <casingItem type="variant">titlecase</casingItem>
      <casingItem type="zone_exemplarCity">titlecase</casingItem>
      <casingItem type="zone_long">titlecase</casingItem>
      <casingItem type="zone_short">titlecase</casingItem>
    </casingData>
  </metadata>
</ldml>
"#
        );
    }

    #[test]
    fn find_identity() {
        let doc = Document::new("tests/en_US.xml").expect("should parse test LDML");
        let silid = doc
            .find_nodes("/ldml/*[self::identity[special/sil:identity] or self::metadata]")
            .expect("should find identity and metadata nodes");
        let res = silid
            .iter()
            .map(|n| doc.inner.ronode_to_string(&n))
            .collect::<Vec<_>>();

        assert_eq!(silid.len(), 2);
        assert_eq!(
            res,
            [
                "<identity>\
                <version number=\"$Revision$\"/>\
                <language type=\"en\"/>\
                <territory type=\"US\"/>\
                <special xmlns:sil=\"urn://www.sil.org/ldml/0.1\">\
                <sil:identity defaultRegion=\"US\" revid=\"b83dea0b8c92193966b10b85c823a22479d1c3ed\" script=\"Latn\" source=\"cldr\"/>\
                </special>\
                </identity>",
                "<metadata>\
                <casingData>\
                <casingItem type=\"calendar_field\">lowercase</casingItem>\
                <casingItem type=\"currencyName\">titlecase</casingItem>\
                <casingItem type=\"currencyName_count\">titlecase</casingItem>\
                <casingItem type=\"day_format_except_narrow\">titlecase</casingItem>\
                <casingItem type=\"day_narrow\">titlecase</casingItem>\
                <casingItem type=\"day_standalone_except_narrow\">titlecase</casingItem>\
                <casingItem type=\"era_abbr\">titlecase</casingItem>\
                <casingItem type=\"era_name\">titlecase</casingItem>\
                <casingItem type=\"era_narrow\">titlecase</casingItem>\
                <casingItem type=\"key\">titlecase</casingItem>\
                <casingItem type=\"keyValue\">titlecase</casingItem>\
                <casingItem type=\"language\">titlecase</casingItem>\
                <casingItem type=\"metazone_long\">titlecase</casingItem>\
                <casingItem type=\"month_format_except_narrow\">titlecase</casingItem>\
                <casingItem type=\"month_narrow\">titlecase</casingItem>\
                <casingItem type=\"month_standalone_except_narrow\">titlecase</casingItem>\
                <casingItem type=\"quarter_abbreviated\">titlecase</casingItem>\
                <casingItem type=\"quarter_format_wide\">titlecase</casingItem>\
                <casingItem type=\"quarter_standalone_wide\">titlecase</casingItem>\
                <casingItem type=\"relative\">lowercase</casingItem>\
                <casingItem type=\"script\">titlecase</casingItem>\
                <casingItem type=\"territory\">titlecase</casingItem>\
                <casingItem type=\"variant\">titlecase</casingItem>\
                <casingItem type=\"zone_exemplarCity\">titlecase</casingItem>\
                <casingItem type=\"zone_long\">titlecase</casingItem>\
                <casingItem type=\"zone_short\">titlecase</casingItem>\
                </casingData>\
                </metadata>"
            ]
        );
    }
}
