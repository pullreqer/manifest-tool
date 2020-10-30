By default it will convert a .repo/manifests/default.xml into a .repo/local_manifests/default.xml
by reading a template is read from ~/.config/manifest-tool/local.env file.
The template should look something like:

```
push_url=ssh://test@example.com:2222/${remote_name}/
review_url=ssh://localhost:2222/
fetch_url=ssh://test@example.com:2222/${remote_name}/
review_proto=gerrit
```

The `-p foo.env` option evaluates a given template for each project
searching for templates in  ~/.config/manifest-tool/projects/foo.env.

Example:
```
cat <<-"EOF" | manifest-tool -P -
echo ${remote_name}/${project_name}
EOF
```

Similarly the `-r foo.env` option evalates a given template for each remote ~/.config/manifest-tool/remote/foo.env
