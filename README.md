# A proxy/translator to connect restic to JottaCloud

Restic is one of the best backup tools, JottaCloud is quite a good _European_ cloud (as in Dropbox) provider. Why not connect both?

## State

Backup and restoring of data is possible. Things are backuped to a subfolder of you `Sync` directory (the one that is synchronized across machines). If you don't want it to be resored to another machine, please exclude this folder.

### TODO

* Downloads are not as efficient as they could be. They are first collected to local memory, before restic will see a byte. That isn't too bad, since restic's blocks are not too big. I need to work out API changes directly pass the stream.
* Uploads have the same problem. But that's an API limitation, since we need to calculate the MD5 of the data to be uploaded beforehand. There seems to be a new API available, but that requires oauth.

## Usage

* Run the rest server:
```
cargo run --release
```
* Call restic, pointing it to the server
```
restic --repo rest:http://localhost:8080/test/  init
restic --repo rest:http://localhost:8080/test/  backup some_folder
```