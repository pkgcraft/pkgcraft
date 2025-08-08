use roxmltree::{Document, ParsingOptions};

/// Parse XML data allowing DTD parsing.
pub(crate) fn parse_xml_with_dtd(data: &str) -> Result<Document<'_>, roxmltree::Error> {
    let opt = ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    Document::parse_with_options(data, opt)
}
