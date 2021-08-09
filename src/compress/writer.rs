use crate::{
	errors::*,
	names::{KnownName, Name},
};
use byteorder::*;
use std::{collections::HashMap, io::Write, sync::Arc};

pub struct CompressWriter<W: Write> {
	out: W,
	string_table_index: HashMap<Arc<str>, u64>,
	string_table: Vec<Arc<str>>,
}
impl<W: Write> CompressWriter<W> {
	pub fn write_varuint(&mut self, data: impl Into<u64>) -> Result<()> {
		let mut data = data.into();
		loop {
			let frag = data & 0x7F;
			data = data >> 7;

			if data == 0 {
				self.out.write_u8(frag as u8)?;
				break;
			} else {
				self.out.write_u8(0x80 | frag as u8)?;
			}
		}
		Ok(())
	}

	pub fn intern(&mut self, str: &str) -> u64 {
		match self.string_table_index.get(str) {
			Some(x) => *x,
			None => {
				let str: Arc<str> = str.into();
				let tok = self.string_table_index.len() as u64;

				self.string_table_index.insert(str.clone(), tok);
				self.string_table.push(str.clone());

				tok
			}
		}
	}

	pub fn write_name(&mut self, name: &Name<'_>) -> Result<()> {
		let id = self.intern(name.as_str());
		self.write_varuint(id)
	}
	pub fn write_known_name(&mut self, name: KnownName) -> Result<()> {
		let id = self.intern(name.as_str());
		self.write_varuint(id)
	}

	pub fn encode_string_name(&mut self, name: &Name<'_>) -> Result<()> {
		if name.is_low_level() {
			self.write_known_name(KnownName::LlNameTable)?;
			self.write_name(name)?;
		} else {
			self.write_name(name)?;
		}
		Ok(())
	}
	pub fn encode_string_full(&mut self, value: &str) -> Result<()> {
		self.write_known_name(KnownName::LlUtf8)?;
		self.write_varuint(value.len() as u64)?;
		self.out.write_all(value.as_bytes())?;
		Ok(())
	}

	pub fn write_arg_string(&mut self, value: &str) -> Result<()> {
		if self.string_table_index.contains_key(value) {
			self.write_arg_string_interned(value)
		} else {
			self.encode_string_full(value)
		}
	}
	pub fn write_arg_string_interned(&mut self, value: &str) -> Result<()> {
		self.encode_string_name(&Name::from(value))
	}
	pub fn write_arg_varuint(&mut self, value: impl Into<u64>) -> Result<()> {
		self.write_known_name(KnownName::LlVarUInt)?;
		self.write_varuint(value)?;
		Ok(())
	}
	pub fn write_arg_bool(&mut self, val: bool) -> Result<()> {
		if val {
			self.write_known_name(KnownName::CoreTrue)
		} else {
			self.write_known_name(KnownName::CoreFalse)
		}
	}
}
