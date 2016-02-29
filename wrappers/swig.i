%module imageflow

/*  How to generate
 * cd wrappers/csharp
 * swig -csharp -outcurrentdir ../swig.i
*/

%nodefaultctor;

%rename("%(camelcase)(strip:[flow_])s") "";

%{

typedef signed char		int8_t;
typedef short int		int16_t;
typedef int			int32_t;
# if __WORDSIZE == 64
typedef long int		int64_t;
# else
__extension__
typedef long long int		int64_t;
# endif


/* Unsigned.  */
typedef unsigned char		uint8_t;
typedef unsigned short int	uint16_t;
typedef unsigned int		uint32_t;
#if __WORDSIZE == 64
typedef unsigned long int	uint64_t;
#else
__extension__
typedef unsigned long long int	uint64_t;
#endif



#include "../fastscaling.h"
/* Includes the header in the wrapper code */
#include "../imageflow.h"

%}

/* Parse the header file to generate wrappers */

%include "../fastscaling.h"
%include "../imageflow.h"
