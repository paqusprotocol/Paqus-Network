use crate::runtime::network::error::NetworkError;
use crate::runtime::network::message::NetworkEnvelope;
use crate::runtime::params::MAX_NETWORK_MESSAGE_SIZE;
use std::io::{ErrorKind, Read, Write};

const MESSAGE_LENGTH_SIZE: usize = 4;

pub fn write_message<W: Write>(
    writer: &mut W,
    envelope: &NetworkEnvelope,
) -> Result<(), NetworkError> {
    let bytes = envelope.to_bytes()?;
    let length = u32::try_from(bytes.len()).map_err(|_| NetworkError::MessageTooLarge)?;

    writer
        .write_all(&length.to_be_bytes())
        .map_err(NetworkError::Io)?;
    writer.write_all(&bytes).map_err(NetworkError::Io)?;
    super::metrics::NETWORK_METRICS.record_tx(&envelope.message, (bytes.len() + 4) as u64);
    Ok(())
}

pub fn read_message<R: Read>(reader: &mut R) -> Result<NetworkEnvelope, NetworkError> {
    let mut length_bytes = [0_u8; MESSAGE_LENGTH_SIZE];
    read_exact_logged(reader, &mut length_bytes, "length")?;
    let length = u32::from_be_bytes(length_bytes) as usize;

    if length > MAX_NETWORK_MESSAGE_SIZE {
        return Err(NetworkError::MessageTooLarge);
    }

    let mut bytes = vec![0_u8; length];
    read_exact_logged(reader, &mut bytes, "payload")?;
    let envelope = NetworkEnvelope::from_bytes(&bytes)?;
    super::metrics::NETWORK_METRICS.record_rx(&envelope.message, (bytes.len() + 4) as u64);
    Ok(envelope)
}

fn read_exact_logged<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    phase: &'static str,
) -> Result<(), NetworkError> {
    let expected = buffer.len();
    let mut received = 0_usize;
    while received < expected {
        match reader.read(&mut buffer[received..]) {
            Ok(0) => {
                return Err(NetworkError::Io(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    format!(
                        "peer closed while reading {phase}: expected_bytes={expected} received_bytes={received}"
                    ),
                )));
            }
            Ok(bytes) => received = received.saturating_add(bytes),
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(NetworkError::Io(std::io::Error::new(
                    error.kind(),
                    format!(
                        "failed while reading {phase}: expected_bytes={expected} received_bytes={received}: {error}"
                    ),
                )));
            }
        }
    }
    Ok(())
}
