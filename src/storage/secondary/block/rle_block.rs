use bytes::{Buf, Bytes};
use rust_decimal::Decimal;

use super::Block;
use crate::array::{
    Array, BlobArray, BoolArray, DateArray, DecimalArray, F64Array, I32Array, IntervalArray,
    Utf8Array,
};
use crate::types::{BlobRef, Date, Interval};

pub enum RLETypeKind {
    /// Primitive Type
    Primitive,

    /// Bytes Type, with char_widtch inside
    Bytes(Option<u64>),
}

pub fn decode_rle_block(data: Block) -> (usize, Block, Block) {
    let mut buffer = &data[..];
    let rle_num = buffer.get_u32_le() as usize;
    let rle_length = std::mem::size_of::<u32>() + std::mem::size_of::<u16>() * rle_num;
    let rle_data = data[std::mem::size_of::<u32>()..rle_length].to_vec();
    let block_data = data[rle_length..].to_vec();
    (rle_num, Bytes::from(rle_data), Bytes::from(block_data))
}

pub trait RLETypeEncode: PartialEq + 'static + Send + Sync {
    const WIDTH: usize;

    type ArrayType: Array<Item = Self>;

    fn len(&self) -> usize {
        Self::WIDTH
    }

    fn to_ref_bytes(&self) -> &[u8] {
        &[0]
    }

    fn to_vec_bytes(&self) -> Vec<u8> {
        vec![0]
    }

    // fn from_ref_bytes(bytes: &[u8]) -> Self {
    //     Self::default()
    // }
}

impl RLETypeEncode for bool {
    const WIDTH: usize = std::mem::size_of::<u8>();

    type ArrayType = BoolArray;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     if *self {
    //         &[1]
    //     } else {
    //         &[0]
    //     }
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<u8>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

impl RLETypeEncode for i32 {
    const WIDTH: usize = std::mem::size_of::<i32>();

    type ArrayType = I32Array;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     // &self.to_le_bytes()
    //     &[0]
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<i32>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl RLETypeEncode for f64 {
    const WIDTH: usize = std::mem::size_of::<f64>();

    type ArrayType = F64Array;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     // self.to_le_bytes().as_ref()
    //     &[0]
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<f64>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl RLETypeEncode for Decimal {
    const WIDTH: usize = std::mem::size_of::<Decimal>();

    type ArrayType = DecimalArray;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     // self.serialize().as_ref()
    //     &[0]
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<Decimal>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        self.serialize().to_vec()
    }
}

impl RLETypeEncode for Date {
    const WIDTH: usize = std::mem::size_of::<i32>();

    type ArrayType = DateArray;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     // self.get_inner().to_le_bytes().as_ref()
    //     &[0]
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<i32>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        self.get_inner().to_le_bytes().to_vec()
    }
}

impl RLETypeEncode for Interval {
    const WIDTH: usize = std::mem::size_of::<i32>() + std::mem::size_of::<i32>();

    type ArrayType = IntervalArray;

    // fn to_ref_bytes(&self) -> &[u8] {
    //     // let mut bytes = [0; 8];
    //     // bytes[0..4].copy_from_slice(&self.num_months().to_le_bytes());
    //     // bytes[4..8].copy_from_slice(&self.days().to_le_bytes());
    //     // bytes.as_ref()
    //     &[0]
    // }

    // fn len(&self) -> usize {
    //     std::mem::size_of::<i32>() + std::mem::size_of::<i32>()
    // }

    fn to_vec_bytes(&self) -> Vec<u8> {
        let mut bytes = [0; 8];
        bytes[0..4].copy_from_slice(&self.num_months().to_le_bytes());
        bytes[4..8].copy_from_slice(&self.days().to_le_bytes());
        bytes.to_vec()
    }
}

impl RLETypeEncode for BlobRef {
    const WIDTH: usize = 0;

    type ArrayType = BlobArray;

    fn to_ref_bytes(&self) -> &[u8] {
        self.as_ref()
    }

    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl RLETypeEncode for str {
    const WIDTH: usize = 0;

    type ArrayType = Utf8Array;

    fn to_ref_bytes(&self) -> &[u8] {
        self.as_bytes()
    }

    fn len(&self) -> usize {
        self.len()
    }
}
