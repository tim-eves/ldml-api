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
    pub fn new<'a, P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path> + 'a,
    {
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

    pub fn findnodes(&self, xpath: &str) -> Option<Vec<RoNode>> {
        let root = self.inner.get_root_readonly()?;
        let ctxt = self.get_context()?;
        ctxt.node_evaluate_readonly(xpath, root)
            .ok()
            .map(|res| res.get_readonly_nodes_as_vec())
    }

    pub fn _findvalue(&self, xpath: &str) -> Option<String> {
        self.get_context()
            .and_then(|mut ctxt| ctxt.findvalue(xpath, None).ok())
    }

    pub fn subset(&mut self, xpaths: &[&str]) -> Result<(), String> {
        let compound =
            "/ldml/*[self::".to_string() + &xpaths.join(" or self::") + " or self::identity]";

        let nodes = self.findnodes(&compound).ok_or("XPath evalution failed")?;
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
}

impl ToString for Document {
    fn to_string(&self) -> String {
        // Default::default()
        self.inner.to_string_with_options(SaveOptions {
            format: true,
            no_empty_tags: true,
            no_xhtml: true,
            non_significant_whitespace: true,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Document;

    #[test]
    fn parse_ldml_doc() {
        let doc = Document::new("test/en_US.xml");

        assert!(doc.is_ok());
    }

    #[test]
    fn find_revid() {
        let doc = Document::new("test/en_US.xml").expect("LDML failed parse.");
        let revid = doc
            ._findvalue("//sil:identity/@revid")
            .expect("revid not found");

        assert_eq!(revid, "b83dea0b8c92193966b10b85c823a22479d1c3ed");
    }

    #[test]
    fn find_sil_kdb() {
        let doc = Document::new("test/en_US.xml").expect("LDML failed parse.");
        let silkbd = doc
            ._findvalue("//sil:kbd[@id = 'basic_kbdusa']/sil:url")
            .expect("Value not found");

        assert_eq!(
            silkbd,
            "https://keyman.com/go/keyboard/basic_kbdusa/download/kmp"
        );
    }

    #[test]
    fn subsetting() {
        let mut doc = Document::new("test/en_US.xml").expect("LDML failed parse.");
        doc.subset(&["metadata", "layout"])
            .expect("Subsetting failed");

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
    <version number="$Revision$"></version>
    <language type="en"></language>
    <territory type="US"></territory>
    <special xmlns:sil="urn://www.sil.org/ldml/0.1">
      <sil:identity defaultRegion="US" revid="b83dea0b8c92193966b10b85c823a22479d1c3ed" script="Latn" source="cldr"></sil:identity>
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
        let doc = Document::new("test/en_US.xml").expect("LDML failed parse.");
        let silid = doc
            .findnodes("/ldml/*[self::identity[special/sil:identity] or self::metadata]")
            .expect("Node not found");
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
