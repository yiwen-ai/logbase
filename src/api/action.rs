const ACTIONS: [&str; 72] = [
    "sys.create.user",
    "sys.update.user",
    "sys.update.group",
    "sys.update.creation",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "user.login",
    "user.authz",
    "user.update",
    "user.update.cn",
    "user.logout",
    "user.collect",
    "user.follow",
    "user.subscribe",
    "user.sponsor",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "group.create",
    "group.update",
    "group.update.cn",
    "group.transfer",
    "group.delete",
    "group.create.user",
    "group.update.user",
    "group.add.member",
    "group.update.member",
    "group.remove.member",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "creation.create",
    "creation.create.converting",
    "creation.create.scraping",
    "creation.update",
    "creation.update.content",
    "creation.release",
    "creation.delete",
    "creation.assist",
    "creation.transfer",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "publication.create",
    "publication.update",
    "publication.update.content",
    "publication.publish",
    "publication.delete",
    "publication.assist",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
];

pub fn from_action(a: i8) -> String {
    if a < 0 || a as usize >= ACTIONS.len() {
        "reserved".to_string()
    } else {
        ACTIONS[a as usize].to_string()
    }
}

pub fn to_action(a: &str) -> Option<i8> {
    if a == "reserved" {
        None
    } else {
        ACTIONS.iter().position(|&x| x == a).map(|x| x as i8)
    }
}
