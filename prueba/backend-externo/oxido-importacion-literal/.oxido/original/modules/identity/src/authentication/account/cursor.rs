use uuid::Uuid;

use super::super::AuthenticationError;

pub(super) fn decode(cursor: Option<&str>) -> Result<Option<(i64, Uuid)>, AuthenticationError> {
    cursor
        .map(|cursor| {
            let (timestamp, identifier) = cursor
                .split_once(':')
                .ok_or(AuthenticationError::InvalidCursor)?;
            let timestamp = timestamp
                .parse::<i64>()
                .map_err(|_| AuthenticationError::InvalidCursor)?;
            let identifier = identifier
                .parse::<Uuid>()
                .map_err(|_| AuthenticationError::InvalidCursor)?;
            Ok((timestamp, identifier))
        })
        .transpose()
}

pub(super) fn encode(timestamp: i64, identifier: Uuid) -> String {
    format!("{timestamp}:{identifier}")
}
