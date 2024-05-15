use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::{fdo, message::Header, Connection};

pub async fn get_user_id(header: Header<'_>) -> fdo::Result<u32> {
    let sender = header
        .sender()
        .ok_or(fdo::Error::AuthFailed("No header".into()))?;
    let credentials = DBusProxy::new(&Connection::system().await?)
        .await?
        .get_connection_credentials(BusName::Unique(sender.to_owned()))
        .await?;
    let user_id = credentials
        .unix_user_id()
        .ok_or(fdo::Error::AuthFailed("No user id in credentials".into()))?;
    Ok(user_id)
}
