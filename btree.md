A B-tree puts multiple sorted keys in each node and branches multiple ways (into the gaps between those keys).

Why do that? Because each step down a tree is a hop from one node to another. A wider node means fewer levels, so you reach your target in fewer hops.

Suppose an internal node holds two sorted keys: 20 and 50. Those two keys divide all possible search values into 3 ranges, so the node has 3 child pointers:

```txt
      [   20   |   50   ]
      /        |        \
  (left)    (middle)    (right)
   < 20     20 to 50     > 50
```

The Two Balance Rules

- For our max-2-keys tree:
  - Key count per node: Every non-root node must hold 1 or 2 keys. (If it has 0 keys, it's empty/underfull. If it tries to hold 3 keys, it's overfull and must split.)
  - Equal leaf depth: Every leaf node must be at the exact same level from the root.

Step-by-Step WalkthroughStart with this valid tree

```txt
        [ 20 | 50 ]
  /         |         \
[ 10 ] [ 30 | 40 ] [ 60 | 70 ]
```

A. Searching for 40
Look at the root [ 20 | 50 ]. 40 is between 20 and 50, so go to the middle child.Look at [ 30 | 40 ]. 40 is right there. Done in 2 nodes.

B. Inserting 35

```txt
          [   20   |   50   ]
          /        |        \
   [ 10 ]     [ 30 | 40 ]   [ 60 | 70 ]

   [   20   |   35   |   50   ]
   /        |        |        \
[ 10 ]   [ 30 ]   [ 40 ]   [ 60 | 70 ]

               [      35      ]
               /              \
        [   20   ]          [   50   ]
        /        \          /        \
   [ 10 ]     [ 30 ]     [ 40 ]   [ 60 | 70 ]
```

C. Deleting 60

```txt
          [   20   |   50   ]
          /        |        \
   [ 10 ]     [ 30 | 40 ]   [ 60 | 70 ]

          [   20   |   50   ]
          /        |        \
   [ 10 ]     [ 30 | 40 ]   [ 70 ]
```

D. Deleting 10

- Deleting 10 leaves [ ] (0 keys).
- Try borrowing from a sibling first: Look at the right sibling [ 30 | 40 ]. It has 2 keys, so it has a spare key and can lend one.
- Rotate through the parent: You can't move 30 directly into the left child without breaking the parent's routing boundary (20), so:Pull the parent separator 20 down into the left child.

```txt
          [   20   |   50   ]
          /        |        \
   [ 10 ]     [ 30 | 40 ]   [ 60 | 70 ]

          [   30   |   50   ]
          /        |        \
   [ 20 ]       [ 40 ]    [ 60 | 70 ]
```

- Why Block Sizing Matters
  Storage devices (SSDs, HDDs, OS page caches) don't read or write a single 8-byte key or pointer in isolation; they transfer data in fixed-size blocks or pages (commonly 4 KB or 16 KB).

A single BST node is tiny (maybe 16–32 bytes: one key, value/record ID, and two pointers). If you read a whole 4 KB block to get a 24-byte node, over 99% of that I/O transfer is wasted on unrelated bytes, and walking down 20–30 levels can trigger dozens of separate block reads.

A B-tree node is sized to fit inside one block/page. Because one block read brings in dozens or hundreds of sorted keys and child pointers at once:

You get high fan-out (many branches per level), keeping total tree height very small (often 3–5 levels).

Each block read loads a rich set of routing keys or leaf entries in one shot instead of paying a separate I/O round-trip per key.
