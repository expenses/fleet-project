use crate::BoundingBox;

#[derive(Debug)]
struct Node<T> {
    bounding_box: BoundingBox,
    data: Option<T>,
    parent_index: Option<usize>,
    left_child: usize,
    right_child: usize,
}

impl<T> Node<T> {
    fn delta_surface_area(&self, bounding_box: BoundingBox) -> f32 {
        self.bounding_box.union_with(bounding_box).surface_area() - self.bounding_box.surface_area()
    }
}

struct MinHeapItem<T> {
    priority: f32,
    data: T,
}

impl<T> PartialEq for MinHeapItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<T> Eq for MinHeapItem<T> {}

impl<T> std::cmp::Ord for MinHeapItem<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .priority
            .partial_cmp(&self.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl<T> std::cmp::PartialOrd for MinHeapItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// See https://box2d.org/files/ErinCatto_DynamicBVH_Full.pdf for details
pub struct DynamicBvh<T> {
    nodes: slab::Slab<Node<T>>,
    root: usize,
    insertion_priority_queue: std::collections::BinaryHeap<MinHeapItem<(usize, f32)>>,
}

impl<T> Default for DynamicBvh<T> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            root: 0,
            insertion_priority_queue: Default::default(),
        }
    }
}

impl<T> DynamicBvh<T> {
    // See https://box2d.org/files/ErinCatto_DynamicBVH_Full.pdf
    pub fn insert(&mut self, data: T, bounding_box: BoundingBox) -> usize {
        let leaf_index = self.nodes.insert(Node {
            bounding_box,
            data: Some(data),
            parent_index: None,
            left_child: 0,
            right_child: 0,
        });

        if self.nodes.len() == 1 {
            self.root = leaf_index;
            return leaf_index;
        }

        // Stage 1: find the best sibling for the new leaf

        let sibling = self.find_best_sibling(bounding_box);

        // Stage 2: create a new parent

        let old_parent = self.nodes[sibling].parent_index;

        let new_parent = self.nodes.insert(Node {
            bounding_box: bounding_box.union_with(self.nodes[sibling].bounding_box),
            parent_index: old_parent,
            left_child: leaf_index,
            right_child: sibling,
            data: None,
        });

        self.nodes[sibling].parent_index = Some(new_parent);
        self.nodes[leaf_index].parent_index = Some(new_parent);

        if let Some(old_parent) = old_parent {
            let old_parent = &mut self.nodes[old_parent];

            if old_parent.left_child == sibling {
                old_parent.left_child = new_parent;
            } else {
                old_parent.right_child = new_parent;
            }
        } else {
            self.root = new_parent;
        }

        // Stage 3: walk back up the tree refitting AABBs
        self.refit(leaf_index);

        leaf_index
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    // Implementation of the 'Branch and Bound' algorithm to find the best sibling
    // for a bounding box via the surface area heuristic.
    // Implemented from https://box2d.org/files/ErinCatto_DynamicBVH_Full.pdf
    fn find_best_sibling(&mut self, bounding_box: BoundingBox) -> usize {
        let mut lowest_cost = f32::INFINITY;
        let mut best_sibling = self.root;

        self.insertion_priority_queue.clear();

        self.insertion_priority_queue
            .push(MinHeapItem { priority: 0.0, data: (self.root, 0.0)});

        while let Some(MinHeapItem {
            data: (index, parent_delta_surface_area),
            ..
        }) = self.insertion_priority_queue.pop()
        {
            let node = &self.nodes[index];

            let cost = node.bounding_box.union_with(bounding_box).surface_area()
                + parent_delta_surface_area;

            if cost < lowest_cost {
                lowest_cost = cost;
                best_sibling = index;
            }

            if node.data.is_none() {
                let delta_surface_area =
                    node.delta_surface_area(bounding_box) + parent_delta_surface_area;

                let lower_bound = bounding_box.surface_area() + delta_surface_area;

                if lower_bound < lowest_cost {
                    self.insertion_priority_queue.push(MinHeapItem {
                        priority: lower_bound,
                        data: (node.left_child, delta_surface_area),
                    });
                    self.insertion_priority_queue.push(MinHeapItem {
                        priority: lower_bound,
                        data: (node.right_child, delta_surface_area),
                    });
                }
            }
        }

        best_sibling
    }

    fn refit(&mut self, index: usize) {
        let mut parent_index = self.nodes[index].parent_index;

        while let Some(index) = parent_index {
            let left_child = self.nodes[index].left_child;
            let right_child = self.nodes[index].right_child;

            self.nodes[index].bounding_box = self.nodes[left_child]
                .bounding_box
                .union_with(self.nodes[right_child].bounding_box);

            parent_index = self.nodes[index].parent_index;
        }
    }

    pub fn get_mut(&mut self, index: usize) -> &mut T {
        self.nodes[index].data.as_mut().unwrap()
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if let Some(parent) = self.nodes[index].parent_index {
            let grandparent = self.nodes[parent].parent_index;

            let new_parent = self.sibling_of(parent, index);

            self.nodes[new_parent].parent_index = grandparent;

            if let Some(grandparent) = grandparent {
                if self.nodes[grandparent].left_child == parent {
                    self.nodes[grandparent].left_child = new_parent;
                } else {
                    self.nodes[grandparent].right_child = new_parent;
                }
            } else {
                self.root = new_parent;
            }

            self.refit(parent);

            self.nodes.remove(parent);
        }

        let node = self.nodes.remove(index);
        let data = node.data.unwrap();

        Some(data)
    }

    fn sibling_of(&mut self, parent: usize, child: usize) -> usize {
        if self.nodes[parent].left_child == child {
            self.nodes[parent].right_child
        } else {
            self.nodes[parent].left_child
        }
    }

    #[inline]
    pub fn find<FN: Fn(BoundingBox) -> bool>(&self, predicate: FN) -> BvhIterator<T, FN> {
        BvhIterator {
            stack: if let Some(node) = self.nodes.get(self.root) {
                vec![node]
            } else {
                Vec::new()
            },
            bvh: self,
            predicate,
        }
    }

    pub fn iter_bounding_boxes(&self) -> impl Iterator<Item = (BoundingBox, bool)> + '_ {
        self.nodes
            .iter()
            .map(|(_, node)| (node.bounding_box, node.data.is_some()))
    }
}

pub struct BvhIterator<'a, T, FN> {
    stack: Vec<&'a Node<T>>,
    bvh: &'a DynamicBvh<T>,
    predicate: FN,
}

impl<'a, T, FN: Fn(BoundingBox) -> bool> Iterator for BvhIterator<'a, T, FN> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            if (self.predicate)(node.bounding_box) {
                match &node.data {
                    Some(data) => return Some(data),
                    None => {
                        self.stack.push(&self.bvh.nodes[node.left_child]);
                        self.stack.push(&self.bvh.nodes[node.right_child]);
                    }
                }
            }
        }

        None
    }
}

struct DebugNodes<'a, T>(&'a slab::Slab<Node<T>>);

impl<'a, T: std::fmt::Debug> std::fmt::Debug for DebugNodes<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_map().entries(self.0.iter()).finish()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for DynamicBvh<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("DynamicBvh")
            .field("nodes", &format_args!("{:?}", DebugNodes(&self.nodes)))
            .field("root", &self.root)
            .finish()
    }
}

#[test]
fn test() {
    use ultraviolet::Vec3;

    let mut bvh = DynamicBvh::default();
    bvh.insert((), BoundingBox::new(-Vec3::one(), Vec3::one()));
    bvh.insert(
        (),
        BoundingBox::new(-Vec3::one(), Vec3::one()) + Vec3::one(),
    );
    //bvh.remove(|_| true);
    dbg!(&bvh);
    panic!();
}
