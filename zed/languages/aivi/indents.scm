; Indentation rules for AIVI

; Increase indent after opening brackets
[
  (do_block "{" @indent)
  (effect_block "{" @indent)
  (generate_block "{" @indent)
  (resource_block "{" @indent)
]

; Decrease indent at closing brackets
[
  (do_block "}" @dedent)
  (effect_block "}" @dedent)
  (generate_block "}" @dedent)
  (resource_block "}" @dedent)
]

; Indent inside lists, tuples, records
[
  (list "[" @indent)
  (list "]" @dedent)
  (tuple_or_group "(" @indent)
  (tuple_or_group ")" @dedent)
  (record "{" @indent)
  (record "}" @dedent)
]
