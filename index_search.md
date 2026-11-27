1. find index definitions
   load table -> schema_cells -> index_record for table_name -> sql -> parse_sql -> create_index_stmt -> Index(name, root_page, columns) -> Vec<Index> -> store in Table

2. check if where clause use index
   where_clause -> filter(column_name, operator, literal) -> match (column_name == literal)? -> find matching index in table? -> scan index
