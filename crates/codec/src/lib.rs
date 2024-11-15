use borsh::io;
use borsh::{BorshDeserialize, BorshSerialize};

const DEFAULT_SERIALIZER_CAPACITY: usize = 1024;
const DEFAULT_MAX_DATA_SIZE: usize = 8 * 1024 * 1024; // 8 MB

fn serialize_with_limit<T>(v: &T, limit: usize) -> io::Result<Vec<u8>>
where
    T: BorshSerialize,
{
    let mut writer = Writer::with_limit(limit);
    v.serialize(&mut writer)?;
    Ok(writer.data)
}

fn deserialize_with_limit<T, B>(bytes: B, limit: usize) -> io::Result<T>
where
    T: BorshDeserialize,
    B: AsRef<[u8]>,
{
    let Some(mut reader) = Reader::with_limit(bytes.as_ref(), limit) else {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "borsh reader max data size limit exceeded",
        ));
    };
    T::try_from_reader(&mut reader)
}

/// Serializes type into bytes without checking the size
pub fn serialize<T>(v: &T) -> io::Result<Vec<u8>>
where
    T: BorshSerialize,
{
    borsh::to_vec(v)
}

/// Deserializes bytes into the type without checking the size
pub fn deserialize<T, B>(bytes: B) -> io::Result<T>
where
    T: BorshDeserialize,
    B: AsRef<[u8]>,
{
    T::try_from_slice(bytes.as_ref())
}

struct Reader<'a> {
    data: &'a [u8],
    read_index: usize,
}

impl<'a> Reader<'a> {
    fn with_limit(data: &'a [u8], limit: usize) -> Option<Self> {
        if data.len() > limit {
            None
        } else {
            Some(Self {
                data,
                read_index: 0,
            })
        }
    }
}

impl<'a> io::Read for Reader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(self.data.len() - self.read_index);
        if len == 0 {
            return Ok(0);
        }

        buf[0..len].copy_from_slice(&self.data[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(len)
    }
}

struct Writer {
    data: Vec<u8>,
    limit: usize,
}

impl Default for Writer {
    fn default() -> Self {
        Self::with_limit(DEFAULT_MAX_DATA_SIZE)
    }
}

impl Writer {
    fn with_limit(limit: usize) -> Self {
        Self {
            data: Vec::with_capacity(DEFAULT_SERIALIZER_CAPACITY),
            limit,
        }
    }
}

impl io::Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.data.len() + buf.len() > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "borsh writer max data size limit exceeded",
            ));
        }

        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct SizeBoundedCodec {
    limit: usize,
}

impl Default for SizeBoundedCodec {
    fn default() -> Self {
        Self {
            limit: DEFAULT_MAX_DATA_SIZE,
        }
    }
}

impl SizeBoundedCodec {
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }

    pub fn serialize<T>(&self, v: &T) -> io::Result<Vec<u8>>
    where
        T: BorshSerialize,
    {
        serialize_with_limit(v, self.limit)
    }

    pub fn deserialize<T, B>(&self, bytes: B) -> io::Result<T>
    where
        T: BorshDeserialize,
        B: AsRef<[u8]>,
    {
        deserialize_with_limit(bytes, self.limit)
    }
}

// TODO: write tests && benchmarks comparing checked and unchecked

#[cfg(test)]
mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};

    use crate::{
        deserialize, deserialize_with_limit, serialize, serialize_with_limit, SizeBoundedCodec,
    };

    #[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
    struct TestStruct {
        a: u64,
        b: String,
        c: Vec<u32>,
        d: [u8; 32],
    }

    #[test]
    fn serde() {
        let t = TestStruct {
            a: 1234,
            b: "hello".to_string(),
            c: vec![4, 16, 23, 99],
            d: [1; 32],
        };
        let bytes = serialize(&t).unwrap();
        let deserialized_t = deserialize(bytes).unwrap();
        assert_eq!(t, deserialized_t);
    }

    #[test]
    fn serde_with_limit() {
        let t = TestStruct {
            a: 1234,
            b: "hello".to_string(),
            c: vec![4, 16, 23, 99],
            d: [1; 32],
        };
        let bytes = serialize_with_limit(&t, 1_000_000).unwrap();

        assert_eq!(serialize_with_limit(&t, bytes.len()).unwrap(), bytes);
        matches!(serialize_with_limit(&t, 1), Err(_));
        matches!(serialize_with_limit(&t, bytes.len() - 1), Err(_));

        assert_eq!(
            deserialize_with_limit::<TestStruct, _>(&bytes, bytes.len()).unwrap(),
            t
        );
        matches!(deserialize_with_limit::<TestStruct, _>(&bytes, 1), Err(_));
        matches!(
            deserialize_with_limit::<TestStruct, _>(&bytes, bytes.len() - 1),
            Err(_)
        );
    }

    #[test]
    fn size_bounded_codec() {
        let v = TestStruct {
            a: 1234,
            b: "hello".to_string(),
            c: vec![4, 16, 23, 99],
            d: [1; 32],
        };

        let codec = SizeBoundedCodec::new(1_000_000);
        let bytes = codec.serialize(&v).unwrap();
        let deserialized_v = codec.deserialize(&bytes).unwrap();
        assert_eq!(v, deserialized_v);

        let codec = SizeBoundedCodec::new(1);
        matches!(codec.serialize(&v), Err(_));
    }

    #[test]
    fn bench() {
        let t = TestStruct {
            a: 1234,
            b: "hello".to_string(),
            c: vec![
                4, 16, 23, 99, 16, 23, 99, 16, 23, 99, 16, 23, 99, 16, 23, 99, 16, 23, 99, 16, 23,
                99,
            ],
            d: [1; 32],
        };
        let bytes = serialize(&t).unwrap();
        let codec = SizeBoundedCodec::default();

        let n = 100_000;

        let start = std::time::Instant::now();
        for _ in 0..n {
            serialize(&t).unwrap();
        }
        println!("serialize: {}", start.elapsed().as_micros());

        let start = std::time::Instant::now();
        for _ in 0..n {
            codec.serialize(&t).unwrap();
        }
        println!("codec serialize: {}", start.elapsed().as_micros());

        let start = std::time::Instant::now();
        for _ in 0..n {
            deserialize::<TestStruct, _>(&bytes).unwrap();
        }
        println!("deserialize: {}", start.elapsed().as_micros());

        let start = std::time::Instant::now();
        for _ in 0..n {
            codec.deserialize::<TestStruct, _>(&bytes).unwrap();
        }
        println!("codec deserialize: {}", start.elapsed().as_micros());
    }
}
