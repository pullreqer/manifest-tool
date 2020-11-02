See the [documentation](https://pullreqer.github.io)

manifest-tool has 3 modes:

`-c`, `-p`, and `-r`,  for `convert`, `projects`, and `remotes`.

### Convert:
Reads an env file: `~/.config/manifest-tool/convert/default.env` by default,
Reads the manifest files in .`repo/manifests`, and replaces the substitutions produced within the env file,
Then it writes the manifest files to `.repo/local_manifests/`.
An example `convert/default.env` is given here:
```
push_url=ssh://test@example.com:2222/${remote_name}/
review_url=ssh://localhost:2222/
fetch_url=ssh://test@example.com:2222/${remote_name}/
review_proto=gerrit
```

### Remotes & Parse,

Remotes and Parse act similarly, the primary difference is between the variables
that are available for substitutions, they merely read a manifest and apply substitions,
writing the result back to stdout.

The `-p foo.env` option evaluates a given template for each project
searching for templates in  ~/.config/manifest-tool/projects/foo.env.

Example:
```
cat <<-"EOF" | manifest-tool -p -
echo ${remote_name}/${project_name}
EOF
```

Will write some stuff to stdout.
