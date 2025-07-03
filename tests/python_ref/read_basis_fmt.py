import basis_set_exchange as bse
import json
import os

assert bse.__version__ == "0.11"

# ## read_basis_fmt

out_root = "read_basis_fmt"
os.makedirs(out_root, exist_ok=True)

cfgs = [
    ("nwchem"        , "cc-pVTZ"    , {"elements": "1, 6-O"    , "fmt": "nwchem"}),
    ("nwchem"        , "def2-ECP"   , {"elements": "49-51"     , "fmt": "nwchem"}),
    ("nwchem"        , "def2-TZVP"  , {"elements": "1-3, 49-51", "fmt": "nwchem"}),
]

# # %%time
for (scene, basis, kwargs) in cfgs:
    token = bse.get_basis(basis, **kwargs, header=False)
    basis_dict = bse.read_formatted_basis_str(token, scene)
    with open(f"{out_root}/{basis}-{scene}.json", "w") as f:
        json.dump(basis_dict, f, indent=2)


