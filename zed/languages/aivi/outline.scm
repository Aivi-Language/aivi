; Document outline for AIVI — surfaces top-level definitions in the symbol list.

; Module declaration
(module_declaration
  path: (module_path) @name) @item

; Top-level type definitions
(type_definition
  name: (upper_identifier) @name) @item

; Top-level type annotations (show as the declared name)
(type_annotation
  name: (lower_identifier) @name) @item

; Top-level bindings (function / value definitions)
(binding
  name: (lower_identifier) @name) @item
