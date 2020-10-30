
The `-l argument transforms a .repo/manifests/*.xml into a .repo/local_manifests/*.xml

a template is read from ~/.config/manifest-tool/config.env file,
```
push_url=ssh://test@example.com:2222/${remote_name}/
review_url=ssh://localhost:2222/
fetch_url=ssh://test@example.com:2222/${remote_name}/
review_proto=gerrit
```

The `-P` option evaluates an envsubst for each project

```
cat <<-"EOF" | manifest-tool -P -
echo ${remote_name}/${project_name}
EOF
```
