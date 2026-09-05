"""Conservative interval analysis of |F| for up to 768 instruction effects.
Keeps one binary64 interval in each exponent binade. Addition/subtraction
allow ANY signed real source of magnitude <= 2**32, with outward rounding.
FSCAL maps exponent bits exactly. This deliberately overapproximates actual
RandomX sources (so this proves an upper bound only, not a lower bound).
"""
import struct,math
bits=lambda x:struct.unpack('<Q',struct.pack('<d',x))[0]
value=lambda b:struct.unpack('<d',struct.pack('<Q',b))[0]
FRAC=(1<<52)-1

def merge(intervals):
 result=[]
 for lo,hi in sorted(intervals):
  if result and lo<=result[-1][1]+1:result[-1]=(result[-1][0],max(hi,result[-1][1]))
  else:result.append((lo,hi))
 return result

def bins(intervals):
 out={}
 for low,high in intervals:
  for e in range(low>>52,(high>>52)+1):
   lo=max(low,e<<52);hi=min(high,(e<<52)|FRAC)
   if e in out:lo=min(lo,out[e][0]);hi=max(hi,out[e][1])
   out[e]=(lo,hi)
 return out
state=bins([(0,bits(2.0**31))])
for step in range(1,769):
 scaled=[(((e^15)<<52)|(lo&FRAC),((e^15)<<52)|(hi&FRAC)) for e,(lo,hi) in state.items()]
 added=[]
 for lo,hi in merge(state.values()):
  a=max(0.0,math.nextafter(value(lo)-2.0**32,-math.inf))
  b=math.nextafter(value(hi)+2.0**32,math.inf)
  assert math.isfinite(b),(step,'addition overflow')
  added.append((bits(a),bits(b)))
 state=bins(merge(scaled+added))
 assert max(state)<2047,(step,'FSCAL reaches NaN/inf')
 if step in [1,2,16,64,256,512,768]:
  hi=max(hi for lo,hi in state.values())
  print(f'step={step}: max |F| <= {value(hi).hex()} (unbiased exponent {hi//2**52-1023})')
print('PASS: conservative F-domain upper bound excludes both infinity and FSCAL-to-NaN for every instruction sequence of length <= 768')
