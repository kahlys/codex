# baywatch

Profile a containerized workload with various CPU configurations.

## Usage

### Get started

1. Build the test docker image:

```none
$ docker build -t myimage testdata
```

2. Run `baywatch`:

```none
$ cargo run -- --image myimage

Docker infos
host ncpu : 6
host memtotal : 2083807232

┌─────┬───────────────┐
│ CPU │ DURATION (ms) │
├─────┼───────────────┤
│ 6   │ 5978          │
│ 5   │ 4984          │
│ 4   │ 3982          │
│ 3   │ 3004          │
│ 2   │ 2003          │
│ 1   │ 1003          │
└─────┴───────────────┘
```
