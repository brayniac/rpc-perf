use super::*;

impl From<&workload::client::Delete> for RequestWithValidator {
    fn from(other: &workload::client::Delete) -> Self {
        DELETE.increment();
        RequestWithValidator {
            request: Some(Request::del(&[other.key.as_ref()])),
            message: None,
            validator: Box::new(validate_delete_response),
        }
    }
}

fn validate_delete_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::Integer(n) => {
            if n.value() > 0 {
                DELETE_DELETED.increment();
            } else {
                DELETE_NOT_FOUND.increment();
            }
            Ok(())
        }
        Response::Error(_) => {
            DELETE_EX.increment();
            Err(())
        }
        _ => {
            DELETE_EX.increment();
            Err(())
        }
    }
}
