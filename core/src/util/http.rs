use reqwest::Response;

pub fn ensure_response(resp: Response) -> Result<Response, rspc::Error> {
	resp.error_for_status()
		.map_err(|e| rspc::Error::new(rspc::ErrorCode::InternalServerError, e.to_string()))
}
