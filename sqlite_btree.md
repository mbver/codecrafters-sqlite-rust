- the number of levels is decided roughly by the number of records and the capacity of a page
  - assume we have 1000 records and each page can hold 10 cells
  - we will need 3 levels: level 1: 1 page: 10 nodes. level 2: 10 pages, 100 nodes, level 3: 100 pages, 1000 nodes

- the calculation is approximate since the size of interior nodes and leaf nodes are different
  - but each interior node points exactly to 1 page
  - we will try to fit a full page with either interior nodes or leaf nodes, there's no mixing of data in a page

- at an interior level, there's n interior nodes and they divide the next level into n + 1 ranges
  - (n-1) node from the left will point exactly 1 page. these are called `left_child`. only the last node has both `left_child` and `right_child`

- a table interior node only hold left_child and a row_id, not real data. an index interior node hold both left_child an real data
