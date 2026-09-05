import hashlib,json,pathlib,subprocess,sys,urllib.request
root=pathlib.Path(__file__).parent
proof_dir=pathlib.Path(sys.argv[1]).resolve()
vkey_path=pathlib.Path(sys.argv[2]).resolve()
gateway='0x397a5f7f3dbd538f23de225b51f532c34448da9b'
proof=(proof_dir/'proof-evm.hex').read_text().strip().removeprefix('0x')
public=(proof_dir/'public-values.hex').read_text().strip().removeprefix('0x')
vkey=vkey_path.read_text().strip()
historical=pathlib.Path(sys.argv[3]).read_text().strip()
assert len(public)==64 and len(proof)==712 and len(vkey)==66

def rpc(url,method,params):
    body=json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params}).encode()
    req=urllib.request.Request(url,data=body,headers={'content-type':'application/json'})
    with urllib.request.urlopen(req,timeout=30) as r:return json.loads(r.read())
def result(url,method,params):
    r=rpc(url,method,params)
    assert 'error' not in r,r
    return r['result']
def calldata(key,values,proof):
    return subprocess.check_output(['cast','calldata','verifyProof(bytes32,bytes,bytes)',key,'0x'+values,'0x'+proof],text=True).strip()
providers=['https://ethereum.publicnode.com','https://rpc.flashbots.net']
assert result(providers[0],'eth_chainId',[])=='0x1'
block=result(providers[0],'eth_getBlockByNumber',['finalized',False])
number=block['number'];block_hash=block['hash']
variants=[('valid',calldata(vkey,public,proof),True),('historical-vkey',calldata(historical,public,proof),False),('changed-public-values',calldata(vkey,f'{int(public[:2],16)^1:02x}'+public[2:],proof),False),('changed-proof',calldata(vkey,public,proof[:-2]+f'{int(proof[-2:],16)^1:02x}'),False)]
report={'gateway':gateway,'chain_id':1,'block_number':int(number,16),'block_hash':block_hash,'program_vkey':vkey,'public_values':public,'broadcast_transaction':False,'providers':[]}
for provider in providers:
    assert result(provider,'eth_chainId',[])=='0x1'
    assert result(provider,'eth_getBlockByNumber',[number,False])['hash']==block_hash
    code=result(provider,'eth_getCode',[gateway,number]);assert code not in ['0x','0x0']
    checked={'rpc':provider,'gateway_code_bytes':len(bytes.fromhex(code[2:])),'gateway_code_sha256':hashlib.sha256(bytes.fromhex(code[2:])).hexdigest(),'calls':[]}
    for name,data,accept in variants:
        response=rpc(provider,'eth_call',[{'to':gateway,'data':data},number])
        if accept:assert response.get('result')=='0x',response
        else:
            assert 'error' in response,response
            assert 'revert' in response['error'].get('message','').lower(),response
        checked['calls'].append({'case':name,'response':response})
        print(provider+' '+name+': '+('accepted' if accept else 'reverted'),flush=True)
    report['providers'].append(checked)
    (root/'evm-verification.json').write_text(json.dumps(report,indent=2)+'\n')
assert report['providers'][0]['gateway_code_sha256']==report['providers'][1]['gateway_code_sha256']
print('Ethereum mainnet proof verification passed on two RPC providers at finalized block '+str(int(number,16)),flush=True)
