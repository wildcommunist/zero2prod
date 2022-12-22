use crate::helpers::{assert_is_redirect_to, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_password_change_form() {
    let app = spawn_app().await;
    let response = app.get_change_password().await;

    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .post_change_password(&serde_json::json!(
            {
                "current_password": Uuid::new_v4().to_string(),
                "new_password": &new_password,
                "new_password_check": &new_password,
            }
        ))
        .await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn new_passwords_fields_must_match() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    // "log in"
    app.post_login(&serde_json::json!(
        {
            "username":&app.test_user.username,
            "password":&app.test_user.password
        }
    ))
    .await;

    // try to change the password
    let response = app
        .post_change_password(&serde_json::json!(
            {
                "current_password":&app.test_user.password,
                "new_password":&new_password,
                "new_password_check":&another_new_password
            }
        ))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Your new passwords do not match</i></p>"));
}

#[tokio::test]
async fn current_password_must_be_valid() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let wrong_password = Uuid::new_v4().to_string();

    // "log in"
    app.post_login(&serde_json::json!(
        {
            "username":&app.test_user.username,
            "password":&app.test_user.password
        }
    ))
    .await;

    let response = app
        .post_change_password(&serde_json::json!(
            {
                "current_password":wrong_password,
                "new_password":&new_password,
                "new_password_check":&new_password
            }
        ))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>You have entered incorrect password</i></p>"));
}
