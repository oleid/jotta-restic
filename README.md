# A proxy/translator to connect restic to JottaCloud

Restic is one of the best backup tools, JottaCloud is quite a good _European_ cloud (as in Dropbox) provider. Why not connect both?

## State

I can query and decode individual objects using a hand-written, bare hyper client.
Apart from that, not much there, yet. Currently I'm writing a server based on actix-web restic can connect to.

As it turned out, an actic-web based client integrates nicely with an actix-web based server (who would have guessed).
It allows me, without too much API hassle, to forward the http request's body data directly to JottaCloud (and vice versa).
So the client will get rewritten in that process.
