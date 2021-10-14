# Log data

Log data should *always* be stored encrypted.

This means:

```bash
echo '[{}]' | age --recipients-file pubkeys.txt > fakedata.json.age
```

And to use it:

```bash
age --decrypt --identity ~/.ssh/id_ed25519 < fakedata.json.age | python do_something.py
```

## Format

`fakedata.json` gives an example of log data in canonical format. In particular,
the schema is given by `../src/log.rs::Action`, and the values are serialized to
one JSON document per line (*not* a JSON array).
