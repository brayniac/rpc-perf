use super::*;

impl From<&workload::client::Get> for RequestWithValidator {
    fn from(other: &workload::client::Get) -> Self {
        GET.increment();
        RequestWithValidator {
            request: Request::get(
                other.key.as_ref(),
            ),
            validator: Box::new(validate_response),
        }
    }
}

pub fn validate_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::BulkString(value) => {
            if value.bytes().is_none() {
                RESPONSE_MISS.increment();
                GET_KEY_MISS.increment();
            } else {
                RESPONSE_HIT.increment();
                GET_KEY_HIT.increment();
            }
            Ok(())
        }
        _ => {
            GET_EX.increment();
            Err(())
        }
    }
}
