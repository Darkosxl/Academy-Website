use anyhow::{Context, Result, bail};
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

pub async fn read<T, R>(reader: &mut R, maximum: usize) -> Result<Option<T>>
where
    T: DeserializeOwned,
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await.context("read NDJSON")?;
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            bail!("NDJSON stream ended in the middle of a message");
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        if line.len() + take > maximum + 1 {
            bail!("NDJSON message exceeds {maximum} bytes");
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            return serde_json::from_slice(&line)
                .context("decode NDJSON message")
                .map(Some);
        }
    }
}

pub async fn write<T, W>(writer: &mut W, value: &T, maximum: usize) -> Result<()>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let encoded = serde_json::to_vec(value).context("encode NDJSON message")?;
    if encoded.len() > maximum {
        bail!("NDJSON message exceeds {maximum} bytes");
    }
    writer.write_all(&encoded).await.context("write NDJSON")?;
    writer.write_all(b"\n").await.context("finish NDJSON")?;
    writer.flush().await.context("flush NDJSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tokio::io::{BufReader, duplex};

    #[tokio::test]
    async fn round_trip_and_eof_are_bounded() {
        let (left, right) = duplex(128);
        let (_, mut left_write) = tokio::io::split(left);
        let mut reader = BufReader::new(right);
        let expected = serde_json::json!({"type":"pong"});
        write(&mut left_write, &expected, 64).await.unwrap();
        drop(left_write);
        assert_eq!(
            read::<Value, _>(&mut reader, 64).await.unwrap(),
            Some(expected)
        );
        assert_eq!(read::<Value, _>(&mut reader, 64).await.unwrap(), None);
    }

    #[tokio::test]
    async fn rejects_an_oversized_line_before_decoding() {
        let (mut writer, reader) = duplex(128);
        tokio::spawn(async move {
            writer.write_all(b"{\"too\":\"long\"}\n").await.unwrap();
        });
        let error = read::<Value, _>(&mut BufReader::new(reader), 8)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("exceeds"));
    }
}
