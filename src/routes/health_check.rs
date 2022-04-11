use actix_web::HttpResponse;

pub async fn health_check() -> HttpResponse {
    println!("in health check");
    HttpResponse::Ok().finish()
}
