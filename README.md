
transforms a .repo/manifests/*.xml into a .repo/local_manifests/*.xml

It uses a ~/.config/manifest-tool/config.env file, config.env can contain substitution variable `${remote_name}` braces required.

```
push_url=ssh://test@example.com:2222/${remote_name}/
review_url=ssh://localhost:2222/
fetch_url=ssh://test@example.com:2222/${remote_name}/
review_proto=gerrit
```
