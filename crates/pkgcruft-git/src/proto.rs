use pkgcruft::report::Report;

tonic::include_proto!("pkgcruft");

impl From<Report> for StringResponse {
    fn from(value: Report) -> Self {
        Self { data: value.to_json() }
    }
}
