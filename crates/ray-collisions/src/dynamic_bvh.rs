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

        self.insertion_priority_queue.push(MinHeapItem {
            priority: 0.0,
            data: (self.root, 0.0),
        });

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
            let (left_child, right_child) = self.children(index);

            self.nodes[index].bounding_box = self.union_of(left_child, right_child);

            self.rotate(index);

            parent_index = self.nodes[index].parent_index;
        }
    }

    // Perform tree rotations from https://box2d.org/files/ErinCatto_DynamicBVH_Full.pdf.
    fn rotate(&mut self, index: usize) {
        debug_assert!(self.nodes[index].data.is_none());

        let (left_child, right_child) = self.children(index);

        let left_grandchildren = if self.nodes[left_child].data.is_none() {
            Some(self.children(left_child))
        } else {
            None
        };

        let right_grandchildren = if self.nodes[right_child].data.is_none() {
            Some(self.children(right_child))
        } else {
            None
        };

        let left_child_sa = self.nodes[left_child].bounding_box.surface_area();
        let right_child_sa = self.nodes[right_child].bounding_box.surface_area();

        match (left_grandchildren, right_grandchildren) {
            (
                Some((left_left_grandchild, left_right_grandchild)),
                Some((right_left_grandchild, right_right_grandchild)),
            ) => {
                // We have 4 possible rotations to choose from, or no rotation
                // if they don't decrease the surface area.

                let l_to_rl = self.union_of(left_child, right_right_grandchild);
                let l_to_rr = self.union_of(left_child, right_left_grandchild);

                let r_to_ll = self.union_of(right_child, left_right_grandchild);
                let r_to_lr = self.union_of(right_child, left_left_grandchild);

                let l_to_rl_sa_delta = l_to_rl.surface_area() - right_child_sa;
                let l_to_rr_sa_delta = l_to_rr.surface_area() - right_child_sa;
                let r_to_ll_sa_delta = r_to_ll.surface_area() - left_child_sa;
                let r_to_lr_sa_delta = r_to_lr.surface_area() - left_child_sa;

                let min_l_to_r_delta = l_to_rl_sa_delta.min(l_to_rr_sa_delta);

                let min_r_to_l_delta = r_to_ll_sa_delta.min(r_to_lr_sa_delta);

                if min_l_to_r_delta < min_r_to_l_delta && min_l_to_r_delta < 0.0 {
                    if min_l_to_r_delta == l_to_rl_sa_delta {
                        self.nodes[right_child].bounding_box = l_to_rl;
                        self.set_left_child(right_child, left_child);
                        self.set_left_child(index, right_left_grandchild);
                    } else {
                        self.nodes[left_child].bounding_box = l_to_rr;
                        self.set_right_child(right_child, left_child);
                        self.set_left_child(index, right_right_grandchild);
                    }
                } else if min_r_to_l_delta < 0.0 {
                    if min_r_to_l_delta == r_to_ll_sa_delta {
                        self.nodes[left_child].bounding_box = r_to_ll;
                        self.set_left_child(left_child, right_child);
                        self.set_right_child(index, left_left_grandchild);
                    } else {
                        self.nodes[left_child].bounding_box = r_to_lr;
                        self.set_right_child(left_child, right_child);
                        self.set_right_child(index, left_right_grandchild);
                    }
                }
            }
            (Some((left_left_grandchild, left_right_grandchild)), None) => {
                // We have 2 possible rotaions to choose from, or no rotation.

                let to_right = self.union_of(right_child, left_left_grandchild);
                let to_right_sa = to_right.surface_area();

                let to_left = self.union_of(right_child, left_right_grandchild);
                let to_left_sa = to_left.surface_area();

                if to_left_sa < to_right_sa && to_left_sa < left_child_sa {
                    self.nodes[left_child].bounding_box = to_left;
                    self.set_left_child(left_child, right_child);
                    self.set_right_child(index, left_left_grandchild);
                } else if to_right_sa < left_child_sa {
                    self.nodes[left_child].bounding_box = to_right;
                    self.set_right_child(left_child, right_child);
                    self.set_right_child(index, left_right_grandchild);
                }
            }
            (None, Some((right_left_grandchild, right_right_grandchild))) => {
                // We have 2 possible rotaions to choose from, or no rotation.

                let to_right = self.union_of(left_child, right_left_grandchild);
                let to_right_sa = to_right.surface_area();

                let to_left = self.union_of(left_child, right_right_grandchild);
                let to_left_sa = to_left.surface_area();

                if to_left_sa < to_right_sa && to_left_sa < right_child_sa {
                    self.nodes[right_child].bounding_box = to_left;
                    self.set_left_child(right_child, left_child);
                    self.set_left_child(index, right_left_grandchild);
                } else if to_right_sa < right_child_sa {
                    self.nodes[right_child].bounding_box = to_right;
                    self.set_right_child(right_child, left_child);
                    self.set_left_child(index, right_right_grandchild);
                }
            }
            (None, None) => {}
        }
    }

    fn set_left_child(&mut self, parent: usize, child: usize) {
        self.nodes[parent].left_child = child;
        self.nodes[child].parent_index = Some(parent);
    }

    fn set_right_child(&mut self, parent: usize, child: usize) {
        self.nodes[parent].right_child = child;
        self.nodes[child].parent_index = Some(parent);
    }

    fn union_of(&self, a: usize, b: usize) -> BoundingBox {
        self.nodes[a]
            .bounding_box
            .union_with(self.nodes[b].bounding_box)
    }

    fn children(&self, parent: usize) -> (usize, usize) {
        let node = &self.nodes[parent];
        (node.left_child, node.right_child)
    }

    pub fn modify_bounding_box_and_refit(&mut self, index: usize, bounding_box: BoundingBox) {
        self.nodes[index].bounding_box = bounding_box;
        self.refit(index);
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
        let (left_child, right_child) = self.children(parent);

        if child == left_child {
            right_child
        } else {
            left_child
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

impl<T: std::fmt::Debug> std::fmt::Debug for DynamicBvh<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut stack = if !self.nodes.is_empty() {
            vec![(self.root, 0)]
        } else {
            Vec::new()
        };
        let mut string = String::new();

        use std::fmt::Write;

        while let Some((index, depth)) = stack.pop() {
            let node = &self.nodes[index];

            write!(
                &mut string,
                "\n{} {}{}",
                " ".repeat(depth * 4),
                index,
                if node.data.is_some() { "*" } else { "" }
            )?;

            if node.data.is_none() {
                stack.push((node.left_child, depth + 1));
                stack.push((node.right_child, depth + 1));
            }
        }

        f.debug_struct("DynamicBvh")
            .field(
                "nodes",
                &format_args!("{}{}", string, if string.is_empty() { "[]" } else { "" }),
            )
            .finish()
    }
}

#[test]
fn test() {
    use ultraviolet::Vec3;

    let bbox = |pos: Vec3| BoundingBox::new(pos - Vec3::broadcast(0.1), pos + Vec3::broadcast(0.1));

    let mut bvh = DynamicBvh::<()>::default();
    for i in 0..100 {
        bvh.insert((), bbox(Vec3::new(i as f32 * 100.0, 0.0, 0.0)));
    }
    dbg!(bvh);
    //panic!("Panicking in order to debug the tree")
}
