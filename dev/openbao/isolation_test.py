import json, sys, time, urllib.request, urllib.error
import os
B=os.environ.get("BAO_ADDR","http://127.0.0.1:18200")+"/v1/"
def call(method, path, body=None, token="root", headers=None):
    req=urllib.request.Request(B+path, method=method, data=json.dumps(body).encode() if body is not None else None)
    req.add_header("X-Vault-Token", token)
    for k,v in (headers or {}).items(): req.add_header(k,v)
    try:
        with urllib.request.urlopen(req) as r:
            raw=r.read(); return r.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as e:
        return e.code, None
call("POST","sys/mounts/csfx-kv",{"type":"kv","options":{"version":"2"}})
call("POST","sys/auth/approle",{"type":"approle"})
A=("rg-a","vault-a"); Bv=("rg-b","vault-b")
def policy(rg,v,scopes):
    return "".join('path "csfx-kv/data/rg/%s/%s/%s" {\n  capabilities = ["read"]\n}\n'%(rg,v,s) for s in scopes)
def setup(rg,v,principal,scopes):
    role="csfx-vault-%s-%s"%(v,principal)
    assert call("PUT","sys/policies/acl/"+role,{"policy":policy(rg,v,scopes)})[0]==204
    assert call("POST","auth/approle/role/"+role,{"token_policies":[role],"token_ttl":"1h","secret_id_num_uses":1,"secret_id_ttl":"10m"})[0]==204
    rid=call("GET","auth/approle/role/%s/role-id"%role)[1]["data"]["role_id"]
    st,sid=call("POST","auth/approle/role/%s/secret-id"%role,None,headers={"X-Vault-Wrap-TTL":"60s"})
    wrap=sid["wrap_info"]["token"]
    st,unw=call("POST","sys/wrapping/unwrap",None,token=wrap)
    st,login=call("POST","auth/approle/login",{"role_id":rid,"secret_id":unw["data"]["secret_id"]},token="")
    return login["auth"]["client_token"], unw["data"]["secret_id"], rid
for rg,v in (A,Bv):
    for n in ("DATABASE_URL","SMTP_HOST","WEBHOOK_KEY"):
        assert call("POST","csfx-kv/data/rg/%s/%s/%s"%(rg,v,n),{"data":{"value":"secret-"+rg+"-"+n}})[0]==200
ta,sida,ridA=setup(*A,"app-a",["*"])
tb,_,_=setup(*Bv,"app-b",["*"])
tsm,_,_=setup(*A,"mailer",["SMTP_*"])
def check(label,got,want):
    print(("PASS" if got==want else "FAIL"),label,got); 
    return got==want
ok=True
ok&=check("A reads own", call("GET","csfx-kv/data/rg/rg-a/vault-a/DATABASE_URL",token=ta)[0],200)
ok&=check("A reads B", call("GET","csfx-kv/data/rg/rg-b/vault-b/DATABASE_URL",token=ta)[0],403)
ok&=check("B reads A", call("GET","csfx-kv/data/rg/rg-a/vault-a/DATABASE_URL",token=tb)[0],403)
ok&=check("A lists B metadata", call("GET","csfx-kv/metadata/rg/rg-b/vault-b/?list=true",token=ta)[0],403)
ok&=check("A lists own metadata", call("GET","csfx-kv/metadata/rg/rg-a/vault-a/?list=true",token=ta)[0],403)
ok&=check("A writes own", call("POST","csfx-kv/data/rg/rg-a/vault-a/X",{"data":{"value":"x"}},token=ta)[0],403)
ok&=check("A deletes own", call("DELETE","csfx-kv/data/rg/rg-a/vault-a/DATABASE_URL",token=ta)[0],403)
ok&=check("mailer reads SMTP_HOST", call("GET","csfx-kv/data/rg/rg-a/vault-a/SMTP_HOST",token=tsm)[0],200)
ok&=check("mailer reads DATABASE_URL", call("GET","csfx-kv/data/rg/rg-a/vault-a/DATABASE_URL",token=tsm)[0],403)
ok&=check("A lists sys policies", call("GET","sys/policies/acl",token=ta)[0],403)
ok&=check("secret-id single use", call("POST","auth/approle/login",{"role_id":ridA,"secret_id":sida},token="")[0],400)
t=time.time()
names=call("GET","csfx-kv/metadata/rg/rg-a/vault-a/?list=true")[1]["data"]["keys"]
for n in names: call("DELETE","csfx-kv/metadata/rg/rg-a/vault-a/"+n)
for role in ("csfx-vault-vault-a-app-a","csfx-vault-vault-a-mailer"):
    call("DELETE","auth/approle/role/"+role); call("DELETE","sys/policies/acl/"+role)
el=time.time()-t
ok&=check("teardown removes secrets", call("GET","csfx-kv/data/rg/rg-a/vault-a/DATABASE_URL")[0],404)
ok&=check("teardown removes list", call("GET","csfx-kv/metadata/rg/rg-a/vault-a/?list=true")[0],404)
ok&=check("old token revoked by role removal (policy gone)", call("GET","csfx-kv/data/rg/rg-a/vault-a/SMTP_HOST",token=tsm)[0],403)
ok&=check("vault B untouched", call("GET","csfx-kv/data/rg/rg-b/vault-b/DATABASE_URL",token=tb)[0],200)
ok&=check("teardown finishes within 60 seconds", el<60, True)
sys.exit(0 if ok else 1)
