set:

let
  prefix = PREFIX;
  blocklist = builtins.fromJSON BLOCKLIST;
  skipToken = SKIP_TOKEN;
in builtins.listToAttrs (map (name: {
  name = prefix + "." + name;
  value = if builtins.elem "${prefix}.${name}" blocklist
          then skipToken
          else (builtins.elemAt (builtins.split "/|-" (builtins.unsafeDiscardStringContext set.${name})) 6);
}) (builtins.attrNames set))
