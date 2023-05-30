use super::*;

impl From<&workload::client::Set> for RequestWithValidator {
    fn from(other: &workload::client::Set) -> Self {
        SET.increment();
        RequestWithValidator {
            request: Request::set(
                other.key.as_ref(),
                other.value.as_ref(),
                None,
                SetMode::Set,
                false,
            ),
            validator: Box::new(validate_response),
        }
    }
}

pub fn validate_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::SimpleString(s) => {
            if s.as_ref() == "OK" {
                SET_STORED.increment();
            } else {
                SET_NOT_STORED.increment();
            }
            
            Ok(())
        }
        _ => {
            SET_EX.increment();
            Err(())
        }
    }
}
