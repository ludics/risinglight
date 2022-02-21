// Copyright 2022 RisingLight Project Authors. Licensed under Apache-2.0.

use std::marker::PhantomData;

use bytes::BufMut;
use risinglight_proto::rowset::BlockStatistics;

use super::{BlockBuilder, RLETypeEncode, RLETypeKind};

/// Encodes fixed-width data into a block with run-length encoding. The layout is
/// rle counts and data from other block builder
/// ```plain
/// | rle_counts_num (u32) | rle_count (u16) | rle_count | data | data | (may be bit) |
/// ```
pub struct RLEBlockBuilder<T, B>
where
    T: RLETypeEncode + ?Sized,
    B: BlockBuilder<T::ArrayType>,
{
    block_builder: B,
    rle_counts: Vec<u16>,
    previous_value: Option<Vec<u8>>,
    target_size: usize,
    rle_type: RLETypeKind,
    _phantom: PhantomData<T>,
}

impl<T, B> RLEBlockBuilder<T, B>
where
    T: RLETypeEncode + ?Sized,
    B: BlockBuilder<T::ArrayType>,
{
    pub fn new(block_builder: B, target_size: usize, rle_type: RLETypeKind) -> Self {
        Self {
            block_builder,
            rle_counts: Vec::new(),
            previous_value: None,
            target_size,
            rle_type,
            _phantom: PhantomData,
        }
    }

    fn append_inner(&mut self, item: Option<&T>) {
        self.previous_value = match self.rle_type {
            RLETypeKind::Primitive => item.map(|x| x.to_vec_bytes()),
            RLETypeKind::Bytes(_) => item.map(|x| x.to_ref_bytes().to_vec()),
        };
        self.block_builder.append(item);
        self.rle_counts.push(1);
    }
}

impl<T, B> BlockBuilder<T::ArrayType> for RLEBlockBuilder<T, B>
where
    T: RLETypeEncode + ?Sized,
    B: BlockBuilder<T::ArrayType>,
{
    fn append(&mut self, item: Option<&T>) {
        let len = self.rle_counts.len();
        if let Some(item) = item {
            if let Some(previous_value) = &self.previous_value {
                match self.rle_type {
                    RLETypeKind::Primitive => {
                        if previous_value[..] == item.to_vec_bytes()
                            && self.rle_counts[len - 1] < u16::MAX
                        {
                            self.rle_counts[len - 1] += 1;
                            return;
                        }
                    }
                    RLETypeKind::Bytes(_) => {
                        if &previous_value[..] == item.to_ref_bytes()
                            && self.rle_counts[len - 1] < u16::MAX
                        {
                            self.rle_counts[len - 1] += 1;
                            return;
                        }
                    }
                }
            }
        } else if self.previous_value.is_none() && len > 0 && self.rle_counts[len - 1] < u16::MAX {
            self.rle_counts[len - 1] += 1;
            return;
        }
        self.append_inner(item);
    }

    fn estimated_size(&self) -> usize {
        self.block_builder.estimated_size()
            + self.rle_counts.len() * std::mem::size_of::<u16>()
            + std::mem::size_of::<u32>()
    }

    fn should_finish(&self, next_item: &Option<&T>) -> bool {
        if let &Some(item) = next_item {
            if let Some(previous_value) = &self.previous_value {
                match self.rle_type {
                    RLETypeKind::Primitive => {
                        if previous_value[..] == item.to_vec_bytes()
                            && self.rle_counts.last().unwrap_or(&0) < &u16::MAX
                        {
                            return false;
                        }
                    }
                    RLETypeKind::Bytes(_) => {
                        if &previous_value[..] == item.to_ref_bytes()
                            && self.rle_counts.last().unwrap_or(&0) < &u16::MAX
                        {
                            return false;
                        }
                    }
                }
            }
        } else if self.previous_value.is_none() && self.rle_counts.last().unwrap_or(&0) < &u16::MAX
        {
            return false;
        }
        let could_not_append_one = match self.rle_type {
            RLETypeKind::Primitive => {
                self.estimated_size() + T::WIDTH + std::mem::size_of::<u16>() > self.target_size
            }
            RLETypeKind::Bytes(Some(char_width)) => {
                self.estimated_size() + char_width as usize + std::mem::size_of::<u16>()
                    > self.target_size
            }
            RLETypeKind::Bytes(None) => {
                self.estimated_size()
                    + next_item.map(|x| x.len()).unwrap_or(0)
                    + std::mem::size_of::<u32>()
                    + std::mem::size_of::<u16>()
                    > self.target_size
            }
        };
        !self.rle_counts.is_empty() && could_not_append_one
    }

    fn get_statistics(&self) -> Vec<BlockStatistics> {
        self.block_builder.get_statistics()
    }

    fn finish(self) -> Vec<u8> {
        let mut encoded_data: Vec<u8> = vec![];
        encoded_data.put_u32_le(self.rle_counts.len() as u32);
        for count in self.rle_counts {
            encoded_data.put_u16_le(count);
        }
        let data = self.block_builder.finish();
        encoded_data.extend(data);
        encoded_data
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::super::{
        PlainBlobBlockBuilder, PlainCharBlockBuilder, PlainPrimitiveBlockBuilder,
        PlainPrimitiveNullableBlockBuilder,
    };
    use super::*;

    #[test]
    fn test_build_rle_primitive_i32() {
        // Test primitive rle block builder for i32
        let builder = PlainPrimitiveBlockBuilder::new(0);
        let mut rle_builder = RLEBlockBuilder::<i32, PlainPrimitiveBlockBuilder<i32>>::new(
            builder,
            20,
            RLETypeKind::Primitive,
        );
        for item in [Some(&1)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&2)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&3)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        assert_eq!(rle_builder.estimated_size(), 4 * 3 + 2 * 3 + 4);
        assert!(!rle_builder.should_finish(&Some(&3)));
        assert!(rle_builder.should_finish(&Some(&4)));
        rle_builder.finish();
    }

    #[test]
    fn test_build_rle_primitive_nullable_i32() {
        // Test primitive nullable rle block builder for i32
        let builder = PlainPrimitiveNullableBlockBuilder::new(0);
        let mut rle_builder = RLEBlockBuilder::<i32, PlainPrimitiveNullableBlockBuilder<i32>>::new(
            builder,
            70,
            RLETypeKind::Primitive,
        );
        for item in [None].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&1)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [None].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&2)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [None].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&3)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [None].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&4)].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [None].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&5)]
            .iter()
            .cycle()
            .cloned()
            .take(u16::MAX as usize * 2)
        {
            rle_builder.append(item);
        }
        assert_eq!(rle_builder.estimated_size(), 11 * 4 + 2 + 11 * 2 + 4);
        assert!(rle_builder.should_finish(&Some(&5)));
        rle_builder.finish();
    }

    #[test]
    fn test_build_rle_char() {
        // Test rle block builder for char
        let builder = PlainCharBlockBuilder::new(0, 40);
        let mut rle_builder = RLEBlockBuilder::<str, PlainCharBlockBuilder>::new(
            builder,
            150,
            RLETypeKind::Bytes(Some(40)),
        );

        let width_40_char = ["2"].iter().cycle().take(40).join("");

        for item in [Some("233")].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some("2333")].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some(&width_40_char[..])].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        assert_eq!(rle_builder.estimated_size(), 40 * 3 + 2 * 3 + 4);
        assert!(!rle_builder.should_finish(&Some(&width_40_char[..])));
        assert!(rle_builder.should_finish(&Some("2333333")));
        rle_builder.finish();
    }

    #[test]
    fn test_build_rle_varchar() {
        // Test rle block builder for varchar
        let builder = PlainBlobBlockBuilder::new(0);
        let mut rle_builder = RLEBlockBuilder::<str, PlainBlobBlockBuilder<str>>::new(
            builder,
            40,
            RLETypeKind::Bytes(None),
        );
        for item in [Some("233")].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some("23333")].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        for item in [Some("2333333")].iter().cycle().cloned().take(30) {
            rle_builder.append(item);
        }
        assert_eq!(rle_builder.estimated_size(), 15 + 4 * 3 + 2 * 3 + 4); // 37
        assert!(rle_builder.should_finish(&Some("23333333")));
        assert!(!rle_builder.should_finish(&Some("2333333")));
        rle_builder.finish();
    }
}
