//! Serde serializer for [stratum v2][Sv2] implemented following [serde tutorial][tutorial]
//!
//! Right now trying to serialize a value that is an invalid Sv2 type will result in a panic so
//! error are catched as soon as possible.
//!
//! [Sv2]: https://docs.google.com/document/d/1FadCWj-57dvhxsnFM_7X806qyvhR0u3i85607bGHxvg/edit
//! [tutorial]: https://serde.rs/data-format.html
//!
use crate::error::{Error, Result};
use serde::{ser, Serialize};

pub struct Serializer<W: std::io::Write> {
    output: W,
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let output: Vec<u8> = vec![];
    let mut serializer = Serializer { output };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub fn to_writer<T, W: std::io::Write>(value: &T, writer: W) -> Result<()>
where
    T: Serialize,
{
    let mut serializer = Serializer { output: writer };
    value.serialize(&mut serializer)?;
    Ok(())
}

impl<'a, W: std::io::Write> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();

    type Error = Error;

    // Associated types for keeping track of additional state while serializing
    // compound data structures like sequences and maps. In this case no
    // additional state is required beyond what is already stored in the
    // Serializer struct.
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    // TODO check endianess
    fn serialize_bool(self, v: bool) -> Result<()> {
        match v {
            true => self.output.write_all(&[1]).map_err(|_| Error::WriteError)?,
            false => self.output.write_all(&[0]).map_err(|_| Error::WriteError)?,
        };
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.output.write_all(&[v]).map_err(|_| Error::WriteError)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.output
            .write_all(&v.to_le_bytes())
            .map_err(|_| Error::WriteError)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.output
            .write_all(&v.to_le_bytes())
            .map_err(|_| Error::WriteError)
    }

    // Serialize string to STR0_255
    fn serialize_str(self, v: &str) -> Result<()> {
        match v.len() {
            l @ 0..=255 => {
                self.output
                    .write_all(&[l as u8])
                    .map_err(|_| Error::WriteError)?;
            }
            _ => return Err(Error::StringLenBiggerThan256),
        };
        self.output
            .write_all(&v.as_bytes())
            .map_err(|_| Error::WriteError)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.output.write_all(v).map_err(|_| Error::WriteError)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // serialize_struct should preserve field order TODO verify it
    // https://users.rust-lang.org/t/order-of-fields-in-serde-json-to-string/48928/3?u=fi3
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }

    ///// UNIMPLEMENTED /////

    fn serialize_i8(self, _v: i8) -> Result<()> {
        todo!()
    }

    fn serialize_i16(self, _v: i16) -> Result<()> {
        todo!()
    }

    fn serialize_i32(self, _v: i32) -> Result<()> {
        todo!()
    }

    fn serialize_i64(self, _v: i64) -> Result<()> {
        todo!()
    }

    fn serialize_u64(self, _v: u64) -> Result<()> {
        todo!()
    }

    fn serialize_f32(self, _v: f32) -> Result<()> {
        todo!()
    }

    fn serialize_f64(self, _v: f64) -> Result<()> {
        todo!()
    }

    fn serialize_char(self, _v: char) -> Result<()> {
        todo!()
    }

    fn serialize_none(self) -> Result<()> {
        todo!()
    }

    fn serialize_some<T>(self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn serialize_unit(self) -> Result<()> {
        todo!()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        todo!()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        todo!()
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        todo!()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        todo!()
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        todo!()
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        todo!()
    }
}

impl<'a, W: std::io::Write> ser::SerializeStruct for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// TODO check if usefull and in case disimplement it!
impl<'a, W: std::io::Write> ser::SerializeSeq for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W: std::io::Write> ser::SerializeTuple for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

///// UNIMPLEMENTED /////

impl<'a, W: std::io::Write> ser::SerializeTupleStruct for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<()> {
        todo!()
    }
}

impl<'a, W: std::io::Write> ser::SerializeTupleVariant for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<()> {
        todo!()
    }
}

impl<'a, W: std::io::Write> ser::SerializeMap for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<()> {
        todo!()
    }
}

impl<'a, W: std::io::Write> ser::SerializeStructVariant for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<()> {
        todo!()
    }
}

///// TEST /////

#[test]
fn test_struct() {
    #[derive(Serialize)]
    struct Test {
        a: u32,
        b: u8,
    }

    let test = Test { a: 456, b: 9 };
    let expected = vec![200, 1, 0, 0, 9];
    assert_eq!(to_bytes(&test).unwrap(), expected);
}
