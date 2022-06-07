use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::FromRequest;
use actix_web::HttpMessage;
use actix_web_lab::middleware::Next;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct UserID(Uuid);

impl std::fmt::Display for UserID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for UserID {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;
    match session.get_user_id().map_err(e500)? {
        Some(user_id) => {
            req.extensions_mut().insert(UserID(user_id));
            next.call(req).await
        }
        None => {
            let resp = see_other("/login");
            let e = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(e, resp).into())
        }
    }
}
