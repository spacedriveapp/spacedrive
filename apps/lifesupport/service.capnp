@0xd94e9aefe678e1e0;

struct ClientAnnouncement {
    peerId @0 :Text; # TODO: Fixed size????
    addresses @1 :List(Text);
}

interface DiscoverySystem {
    publishAnnouncement @0 (clientAnnouncement :ClientAnnouncement);
    queryAnnouncement @1 (peerId :Text) -> (clientAnnouncement :ClientAnnouncement);
}

interface Proxy {}