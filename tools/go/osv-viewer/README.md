# osv-viewer

Helper tool to display basic informations from a [osv-scanner](https://github.com/google/osv-scanner) json output file.

## Instalation

```sh
go install github.com/kahlys/osv-viewer@v1.0.0
```

## Usage

Use the [osv-scanner](https://github.com/google/osv-scanner) tool on your project and retrieve a json output file.

- You can list all sources with vulnerabilities

```sh
osv-viewer --in <json_file> sources
```

- You can display all vulnerabilities of a source

```sh
osv-viewer --in <json_file> show <source_id>
```
