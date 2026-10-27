# basespy

A tool to reveal hidden datas in base32/base64 encoded strings.

## Usage

### Basic usage

The tool takes as arguments:

- `in` : a file with encoded strings
- `sep` : the separator character of encoded strings (default newline)
- `base` : encoding type (**32** for base32 or **64** for base64)

You can read help:

```none
basespy -h
```

### Example

```none
$ basespy -in examples/encoded.txt 
Read file examples/encoded.txt
Searching for hidden datas

Base_sixty_four_point_five
```
