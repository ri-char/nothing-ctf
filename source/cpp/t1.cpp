
#include <cstdint>
#include <cstdlib>
#include <stdio.h>


void op(int8_t op){
  #include "dwraf.c"
  try{
    if(op>=0)
      throw 1;
  }catch(char*){
  }
  
  return;
}

int main(){
  uint8_t opcode []={3,6,7,0,1,2,4,(uint8_t)-5,3,7,5};
  uint64_t a,b,old_a,old_b;
  scanf("%lx%lx",&a,&b);
  old_a=a;
  old_b=b;
  asm(
    "movq %0,%%r14\n"
    "movq %1,%%r15\n"
    ::"r"(a),"r"(b):"r15","r14"
  );
  uint64_t i=0;
  asm("xor %%r13,%%r13":::"r13");
  while(1){
    try{
      asm("movq %0,%%r12"::"m"(opcode[i]):"r12");
      op(opcode[i]);
    }catch(int a){
    }
    uint64_t r12;
    asm("mov %%r12,%0":"=m"(r12)::"r12");
    if(r12==0||(i+=r12)>=sizeof(opcode)/sizeof(uint8_t)){
      break;
    }
  }
  uint64_t check;
  asm("movq %%r13,%0\n":"=r"(check)::"r13");
  if(check==0){
    printf("Success! Your flag is flag{%016lx%016lx}\n",old_a,old_b);
  }else{
    printf("Error\n");
  }
  return 0;
}
