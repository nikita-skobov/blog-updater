# context-based-variable-substitution

> replace parameter substitution syntax with words given a context

# Example (in pseudo code)

```
my_context = { name: "bob" }
my_string = "this is ${{ name }}, hello!"
replaced = replace_all_from(my_string, my_context, ...)
# replaced = "this is bob, hello!"
```

# Example of defaults (in pseudo code)

```
my_context = {}
my_string = "this is ${{ name | DEFAULT NAME }}, hello!"
replaced = replace_all_from(my_string, my_context, ...)
# replaced = "this is DEFAULT NAME, hello!"
```

# Example of dynamic defaults (in pseudo code)

```
my_context = { dad: "tim" }
# since there is no 'name' key in the context,
# it will use 'dad' as the key, and the context does
# have a 'dad' key, which resolves to 'tim'
my_string = "this is ${{ name || dad }}, hello!"
replaced = replace_all_from(my_string, my_context, ...)
# replaced = "this is tim, hello!"
```

# Example of alternate syntax characters (in pseudo code)

```
my_context = { name: "bob" }
my_string = "this is ${{ name }}, hello!"
replaced = replace_all_from(my_string, my_context, ..., "!?")

# this does not replace anything because we specify that
# the only valid syntax characters are '!' and '?'
# replaced = "this is ${{ name }}, hello!"

# now we change my_string to use one of the valid chars:
my_string = "this is !{{ name }}, hello!"
replaced = replace_all_from(my_string, my_context, ..., "!?")
# replaced = "this is bob, hello!"
```

For full documentation, and real code examples, see the
`replace_all_from` function in [src/lib.rs](./src/lib.rs)
