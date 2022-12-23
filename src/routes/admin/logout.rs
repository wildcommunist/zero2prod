use crate::session_state::TypedSession;
use crate::utils::see_other;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    session.log_out();
    FlashMessage::info("You have been logged out.").send();
    Ok(see_other("/login"))
}
