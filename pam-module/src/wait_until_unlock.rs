use pollster::FutureExt;
use zbus::export::futures_util::StreamExt;
use zbus::message::Type;
use zbus::{MatchRule, MessageStream};

/// This function exits after unlock
pub fn wait_until_unlock() -> zbus::Result<()> {
    Ok({
        let connection = zbus::connection::Connection::session().block_on()?;
        let match_rule = MatchRule::builder()
            .member("ActiveChanged")?
            .interface("org.freedesktop.ScreenSaver")?
            .path("/ScreenSaver")?
            .msg_type(Type::Signal)
            .build();
        let mut stream = Box::pin(
            MessageStream::for_match_rule(match_rule, &connection, None)
                .block_on()?
                .filter_map(|result| async move { result.ok() })
                .map(|message| message.body().deserialize::<bool>())
                .filter_map(|result| async move { result.ok() })
                .filter(|&message| async move { !message }),
        );
        stream.next().block_on();
    })
}
