# Nix Flake Workflow
A reusable [Github Actions](https://docs.github.com/en/actions) workflow for checking and building flakes.
It covers static analysis of the nix code, building all outputs of a flake and optionally pushing them to an [Attic binary cache](https://github.com/zhaofengli/attic).


## Adding the Workflow to a Repository
The easiest way to add the workflow is running the `add` flake runnable:
```sh
nix run github:jzbor/nix-flake-workflow#add
```
This will copy the [default template](./template.yml) to `.github/workflows/flake.yml`.

There is also a `-fast` version, which combines all checking jobs with the "discover" job.
This way the whole setup procedure is only run once and there is less overhead for these typically shorter running tasks.
You can use this variant with the `add-fast` runnable:
```sh
nix run github:jzbor/nix-flake-workflow#add-fast
```


## Setting a Attic Binary Cache
First [generate an access token for the corresponding binary cache](https://docs.attic.rs/tutorial.html) and make sure it exists on the server.
Then add the following secrets to the repository:
* `ATTIC_ENDPOINT`: the endpoint of your cache (e.g. https://cache.jzbor.de)
* `ATTIC_CACHE`: the name of the cache that will be used (e.g. `public`)
* `ATTIC_TOKEN`: the access token generated in the first step

Then you can add or uncomment these lines from the workflow file:
```yml
jobs:
  call:
    with:
      binary-cache: true
    secrets:
      ATTIC_ENDPOINT: ${{ secrets.ATTIC_ENDPOINT }}
      ATTIC_CACHE: ${{ secrets.ATTIC_CACHE }}
      ATTIC_TOKEN: ${{ secrets.ATTIC_TOKEN }}
```

To advertise the cache to users of your flake add the following output attributes:
```Nix
nixConfig = {
  extra-substituters = [ "https://<cache-endpoint>/<cache-name>" ];
  extra-trusted-public-keys = [ "<public-cache-key>" ];
};
```
