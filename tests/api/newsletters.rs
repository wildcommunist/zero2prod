use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use std::time::Duration;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockBuilder, ResponseTemplate};

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    //let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(&serde_json::json!(
        {
            "name":name,
            "email":email
        }
    ))
    .unwrap();

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

async fn create_confirmed_subscribers(app: &TestApp, num: u16) {
    for _ in 0..num {
        create_confirmed_subscriber(app).await;
    }
}

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;
    let response = app
        .post_newsletters(&serde_json::json!(
            {
                    "title":"Newsletter title",
                    "plain":"Newsletter as plan text",
                    "html":"Newsletter as <b>html</b>",
                    "idempotency_key": uuid::Uuid::new_v4().to_string()
                }
        ))
        .await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    let response = app.with_login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "plain":"Newsletter as plain text",
        "html":"Newsletter as <b>html</b>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletter");
    let html = app.get_newsletter_html().await;
    dbg!(&html);
    assert!(html.contains("The newsletter issue has been accepted - emails will go out shortly!"));
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    let response = app.with_login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    create_confirmed_subscribers(&app, 5).await;

    Mock::given(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(5)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "plain":"Newsletter as plain text",
        "html":"Newsletter as <b>html</b>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletter");

    let page_html = app.get_newsletter_html().await;
    assert!(
        page_html.contains("The newsletter issue has been accepted - emails will go out shortly!")
    );
}

#[tokio::test]
async fn newsletter_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let response = app.with_login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let test_cases = vec![
        (
            serde_json::json!(
            {
                "plain": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>",
            }),
            "missing title",
        ),
        (
            serde_json::json!({"title": "Newsletter!"}),
            "missing content",
        ),
        (
            serde_json::json!(
            {
                "title": "Newsletter!",
                "html": "<p>Newsletter body as HTML</p>",
            }),
            "missing plain content",
        ),
        (
            serde_json::json!(
            {
                "title": "Newsletter!",
                "plain": "Newsletter body as plain text",
            }),
            "missing html content",
        ),
        (
            serde_json::json!(
            {
                "title": "Newsletter!",
                "plain": "Newsletter body as plain text",
                "html":"Newsletter in <b>html</b>"
            }),
            "missing idempotency key",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(&invalid_body).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with request status code of 400 when payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let app = spawn_app().await;
    let response = app.with_login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    create_confirmed_subscriber(&app).await;

    Mock::given(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "plain":"Newsletter as plain text",
        "html":"Newsletter as <b>html</b>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletter");

    let page_html = app.get_newsletter_html().await;
    dbg!(&page_html);
    assert!(
        page_html.contains("The newsletter issue has been accepted - emails will go out shortly!")
    );

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletter");

    let page_html = app.get_newsletter_html().await;
    dbg!(&page_html);
    assert!(
        page_html.contains("The newsletter issue has been accepted - emails will go out shortly!")
    );
}

#[tokio::test]
async fn concurrent_form_submission_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    let response = app.with_login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "plain":"Newsletter as plain text",
        "html":"Newsletter as <b>html</b>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response1 = app.post_newsletters(&newsletter_request_body);
    let response2 = app.post_newsletters(&newsletter_request_body);
    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(
        response1.text().await.unwrap(),
        response2.text().await.unwrap()
    );
}

fn when_sending_email() -> MockBuilder {
    Mock::given(path("/email")).and(method("POST"))
}

#[tokio::test]
async fn transient_errors_do_not_cause_duplicate_deliveries_on_retried() {
    let app = spawn_app().await;
    let newsletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "plain":"Newsletter as plain text",
        "html":"Newsletter as <b>html</b>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    create_confirmed_subscriber(&app).await;
    create_confirmed_subscriber(&app).await;
    app.with_login().await;

    // Delivery for first confirmed subscriber "successful"
    when_sending_email()
        .respond_with(ResponseTemplate::new(200))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Fail on the second user
    when_sending_email()
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(&newsletter_request_body).await; // As there are two confirmed subscribers, we have two mocks to consume them, with the very last one returning 500
    assert_eq!(response.status().as_u16(), 500);

    when_sending_email()
        .respond_with(ResponseTemplate::new(200))
        .up_to_n_times(1)
        .expect(1)
        .named("Delivery retry")
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_eq!(response.status().as_u16(), 303);
}
