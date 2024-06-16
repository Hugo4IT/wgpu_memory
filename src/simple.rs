use std::{cmp::Ordering, marker::PhantomData, ops::Range};

use humansize::{format_size, DECIMAL};
use itertools::Itertools;
use slotmap::{DefaultKey, SlotMap};
use wgpu::util::DeviceExt;

use crate::{upload_or_resize, GpuMemory};

/// An index into a list of address ranges in the buffer
pub type AddressId = DefaultKey;
pub type AddressRange = Range<usize>;

/// Uses a normal buffer, adding `COPY_DST` to the buffer usages.
#[derive(Debug)]
pub struct SimpleGpuMemory<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern> {
    buffer: wgpu::Buffer,
    data: Vec<u8>,
    available_ranges: Vec<AddressRange>,
    used_ranges: SlotMap<AddressId, AddressRange>,
    allocated_count: usize,

    mutated: bool,
    _phantom: PhantomData<T>,
}

impl<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern> SimpleGpuMemory<T> {
    fn merge_available_ranges(&mut self, index: usize) {
        while index + 1 < self.available_ranges.len()
            && self.available_ranges[index].end >= self.available_ranges[index + 1].start
        {
            let right = self.available_ranges.remove(index + 1);
            let left = &mut self.available_ranges[index];

            left.start = left.start.min(right.start);
            left.end = left.end.max(right.end);
        }
    }

    fn make_range_available(&mut self, range: AddressRange) {
        if let Some(other_range_index) = self
            .available_ranges
            .iter()
            .position(|other_range| other_range.start <= range.end)
        {
            self.available_ranges.insert(other_range_index, range);
            self.merge_available_ranges(other_range_index);
        } else {
            self.available_ranges.push(range);
        }
    }

    /// Remove all the holes between memory segments
    fn fix_sequence(&mut self) {
        for range in self.available_ranges.drain(..).rev() {
            let range_len = range.len();

            for used_range in self.used_ranges.values_mut() {
                if range.end <= used_range.start {
                    used_range.start -= range_len;
                    used_range.end -= range_len;
                }
            }

            self.data.drain(range);
        }
    }

    fn sort(&mut self, descending: bool) {
        fn _sort_asc((_key, range): &(DefaultKey, &AddressRange)) -> isize {
            range.len() as isize
        }

        fn _sort_desc((_key, range): &(DefaultKey, &AddressRange)) -> isize {
            -(range.len() as isize)
        }

        let ranges = self.used_ranges.clone();
        let sorted_ranges = ranges
            .iter()
            .sorted_by_key(if descending { _sort_desc } else { _sort_asc })
            .collect::<Vec<_>>();

        let mut new_data = Vec::with_capacity(self.size());

        for (key, range) in sorted_ranges {
            let start = new_data.len();
            new_data.extend(&self.data[range.to_owned()]);
            let end = new_data.len();

            self.used_ranges[key] = start..end;
        }

        self.data = new_data;
        self.available_ranges.clear();
    }
}

/// - `Truncate`: delete unused memory and resize the buffer to the smallest
///   possible size to fit allocated items
/// - `SortSizeDescending`: sorts allocated memory regions by their length
///   from longest to shortest. This does not save any memory by itself,
///   however it could make some future operations faster based on the kind
///   of data stored in the buffer.
/// - `SortSizeAscending`: sorts allocated memory regions by their length
///   from shortest to longest. This does not save any memory by itself,
///   however it could make some future operations faster based on the kind
///   of data stored in the buffer.
#[derive(Debug, Clone, Copy, Default)]
pub enum Strategy {
    #[default]
    Truncate,
    SortSizeDescending,
    SortSizeAscending,
}

impl core::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Strategy::Truncate => "Truncate",
                Strategy::SortSizeDescending => "SortSizeDescending",
                Strategy::SortSizeAscending => "SortSizeAscending",
            }
        )
    }
}

impl<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern> GpuMemory<T> for SimpleGpuMemory<T> {
    type Index = AddressId;
    type OptimizationStrategy = Strategy;

    fn new(usages: wgpu::BufferUsages, device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wgpu_text Buffer Allocator"),
            size: core::mem::size_of::<T>() as wgpu::BufferAddress,
            usage: usages | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            data: Vec::new(),
            available_ranges: Vec::new(),
            used_ranges: SlotMap::new(),
            allocated_count: 0,
            mutated: false,
            _phantom: Default::default(),
        }
    }

    fn mutated(&self) -> bool {
        self.mutated
    }

    fn allocate(&mut self, count: usize) -> Self::Index {
        self.mutated = true;

        let size = core::mem::size_of::<T>() * count;

        let range = if let Some(range_index) = self
            .available_ranges
            .iter()
            // Workaround for .rev().position() not really working as expected
            .enumerate()
            .rev()
            .find_map(|(i, range)| (range.len() >= size).then_some(i))
        {
            // If range isn't exactly `size` in length, split it
            if self.available_ranges[range_index].len() != size {
                let range = &mut self.available_ranges[range_index];

                let new_range_end = range.end;
                range.end -= size;
                let new_range_start = range.end;

                new_range_start..new_range_end
            } else {
                self.available_ranges.remove(range_index)
            }
        } else {
            let start = self.data.len();
            self.data.extend((0..size).map(|_| 0));
            let end = self.data.len();

            start..end
        };

        self.allocated_count += count;
        self.used_ranges.insert(range)
    }

    fn len(&self) -> usize {
        self.allocated_count
    }

    fn get(&mut self, index: &Self::Index) -> &mut [T] {
        self.mutated = true;

        let range = &self.used_ranges[*index];

        bytemuck::cast_slice_mut(&mut self.data[range.start..range.end])
    }

    fn len_of(&self, index: &Self::Index) -> usize {
        self.used_ranges[*index].len() / core::mem::size_of::<T>()
    }

    fn resize(&mut self, index: &mut Self::Index, len: usize) {
        let size = len * core::mem::size_of::<T>();

        let range = self.used_ranges[*index].clone();

        match self.used_ranges[*index].len().cmp(&size) {
            Ordering::Less => {
                self.free(*index);
                *index = self.allocate(len);
            }
            Ordering::Equal => (),
            Ordering::Greater => {
                self.mutated = true;

                let free_range = range.start..(range.end - size);
                self.allocated_count -= free_range.len() / core::mem::size_of::<T>();
                self.make_range_available(free_range);

                self.used_ranges[*index].start = range.end - size;
            }
        }
    }

    fn free(&mut self, index: Self::Index) {
        self.mutated = true;

        if let Some(range) = self.used_ranges.remove(index) {
            self.allocated_count -= range.len() / core::mem::size_of::<T>();

            self.make_range_available(range);
        }
    }

    fn upload(&mut self, queue: &wgpu::Queue, device: &wgpu::Device) {
        if !self.mutated {
            return;
        }

        self.fix_sequence();

        upload_or_resize(queue, device, &mut self.buffer, &self.data);

        self.mutated = false;
    }

    fn optimize(
        &mut self,
        strategy: Self::OptimizationStrategy,
        _queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        let size = self.size();

        match strategy {
            Strategy::Truncate => {
                self.fix_sequence();

                log::trace!(
                    "Truncating GPU buffer of size {} to {}",
                    format_size(self.buffer.size(), DECIMAL),
                    format_size(size, DECIMAL)
                );

                self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("wgpu_text Resized Buffer"),
                    usage: self.buffer.usage() | wgpu::BufferUsages::COPY_DST,
                    contents: &self.data,
                });

                let capacity_before = self.data.capacity();

                self.data.shrink_to_fit();

                if capacity_before != self.data.capacity() {
                    log::trace!(
                        "Truncated CPU buffer of size {} to {}",
                        format_size(capacity_before, DECIMAL),
                        format_size(self.data.capacity(), DECIMAL)
                    );
                }
            }
            Strategy::SortSizeDescending => {
                self.sort(true);
            }
            Strategy::SortSizeAscending => {
                self.sort(false);
            }
        }
    }

    fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    fn buffer_slice(&self) -> wgpu::BufferSlice {
        self.buffer.slice(..(self.size() as u64))
    }
}
