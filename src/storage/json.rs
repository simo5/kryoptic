// Copyright 2024 Simo Sorce
// See LICENSE.txt file for terms

use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use serde_json::{
    from_reader, to_string, to_string_pretty, Map, Number, Value,
};

use super::attribute;
use super::err_rv;
use super::error;
use super::interface;
use super::object;
use super::token;

use attribute::{AttrType, Attribute};
use error::{KError, KResult};
use interface::*;
use object::Object;

fn to_json_value(a: &Attribute) -> Value {
    match a.get_attrtype() {
        AttrType::BoolType => match a.to_bool() {
            Ok(b) => Value::Bool(b),
            Err(_) => Value::Null,
        },
        AttrType::NumType => match a.to_ulong() {
            Ok(l) => Value::Number(Number::from(l as u64)),
            Err(_) => Value::Null,
        },
        AttrType::StringType => match a.to_string() {
            Ok(s) => Value::String(s),
            Err(_) => Value::String(BASE64.encode(a.get_value())),
        },
        AttrType::BytesType => Value::String(BASE64.encode(a.get_value())),
        AttrType::DateType => match a.to_date_string() {
            Ok(d) => Value::String(d),
            Err(_) => Value::String(String::new()),
        },
        AttrType::IgnoreType => Value::Null,
        AttrType::DenyType => Value::Null,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonObject {
    attributes: Map<String, Value>,
}

impl JsonObject {
    pub fn from_object(o: &Object) -> JsonObject {
        let mut jo = JsonObject {
            attributes: Map::new(),
        };
        for a in o.get_attributes() {
            jo.attributes.insert(a.name(), to_json_value(a));
        }
        jo
    }

    pub fn rough_size(&self) -> KResult<usize> {
        match to_string(&self) {
            Ok(js) => Ok(js.len()),
            Err(_) => err_rv!(CKR_GENERAL_ERROR),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonToken {
    objects: Vec<JsonObject>,
}

impl JsonToken {
    pub fn load(filename: &String) -> KResult<JsonToken> {
        match std::fs::File::open(filename) {
            Ok(f) => match from_reader::<std::fs::File, JsonToken>(f) {
                Ok(jt) => Ok(jt),
                Err(e) => Err(KError::JsonError(e)),
            },
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    err_rv!(CKR_CRYPTOKI_NOT_INITIALIZED)
                }
                _ => Err(KError::FileError(e)),
            },
        }
    }

    pub fn to_objects(&self, to: &mut token::TokenObjects) -> KResult<()> {
        for jo in &self.objects {
            let mut obj = Object::new();
            let mut uid: Option<String> = None;
            for (key, val) in &jo.attributes {
                let (id, atype) = attribute::attr_name_to_id_type(key)?;
                let attr = match atype {
                    AttrType::BoolType => match val.as_bool() {
                        Some(b) => attribute::from_bool(id, b),
                        None => return err_rv!(CKR_ATTRIBUTE_VALUE_INVALID),
                    },
                    AttrType::NumType => match val.as_u64() {
                        Some(n) => attribute::from_ulong(id, n as CK_ULONG),
                        None => return err_rv!(CKR_ATTRIBUTE_VALUE_INVALID),
                    },
                    AttrType::StringType => match val.as_str() {
                        Some(s) => attribute::from_string(id, s.to_string()),
                        None => return err_rv!(CKR_ATTRIBUTE_VALUE_INVALID),
                    },
                    AttrType::BytesType => match val.as_str() {
                        Some(s) => {
                            let len = match BASE64.decode_len(s.len()) {
                                Ok(l) => l,
                                Err(_) => return err_rv!(CKR_GENERAL_ERROR),
                            };
                            let mut v = vec![0; len];
                            match BASE64.decode_mut(s.as_bytes(), &mut v) {
                                Ok(l) => {
                                    attribute::from_bytes(id, v[0..l].to_vec())
                                }
                                Err(_) => return err_rv!(CKR_GENERAL_ERROR),
                            }
                        }
                        None => return err_rv!(CKR_ATTRIBUTE_VALUE_INVALID),
                    },
                    AttrType::DateType => match val.as_str() {
                        Some(s) => {
                            if s.len() == 0 {
                                /* special case for default empty value */
                                attribute::from_date_bytes(id, Vec::new())
                            } else {
                                attribute::from_date(
                                    id,
                                    attribute::string_to_ck_date(&s)?,
                                )
                            }
                        }
                        None => return err_rv!(CKR_ATTRIBUTE_VALUE_INVALID),
                    },
                    AttrType::DenyType => continue,
                    AttrType::IgnoreType => continue,
                };

                obj.set_attr(attr)?;
                if key == "CKA_UNIQUE_ID" {
                    uid = match val.as_str() {
                        Some(s) => Some(s.to_string()),
                        None => return err_rv!(CKR_DEVICE_ERROR),
                    }
                }
            }
            match uid {
                Some(u) => to.insert(u, obj),
                None => return err_rv!(CKR_DEVICE_ERROR),
            }
        }
        Ok(())
    }

    pub fn from_objects(to: &token::TokenObjects) -> JsonToken {
        let mut jt = JsonToken {
            objects: Vec::with_capacity(to.len()),
        };

        for (_, o) in to.iter() {
            if !o.is_token() {
                continue;
            }
            jt.objects.push(JsonObject::from_object(o));
        }

        jt
    }

    pub fn save(&self, filename: &String) -> KResult<()> {
        let jstr = match to_string_pretty(&self) {
            Ok(j) => j,
            Err(e) => return Err(KError::JsonError(e)),
        };
        match std::fs::write(filename, jstr) {
            Ok(_) => Ok(()),
            Err(e) => Err(KError::FileError(e)),
        }
    }
}
