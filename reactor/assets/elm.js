(function(scope){
'use strict';

function _Rust_curry(arity,fun){
  function step(reverse,count){
    return function(value){
      var values={a:value,b:reverse};
      if(count+1<arity)return step(values,count+1);
      var args=new Array(arity);
      for(var i=arity;i--;values=values.b)args[i]=values.a;
      return fun.apply(null,args);
    };
  }
  return step(null,0);
}
function F2(fun){var wrapper=function(a0){return function(a1){return fun(a0,a1);};};wrapper.a=2;wrapper.f=fun;return wrapper;}
function A2(fun,a0,a1){return fun.a===2?fun.f(a0,a1):fun(a0)(a1);}
function F3(fun){var wrapper=function(a0){return function(a1){return function(a2){return fun(a0,a1,a2);};};};wrapper.a=3;wrapper.f=fun;return wrapper;}
function A3(fun,a0,a1,a2){return fun.a===3?fun.f(a0,a1,a2):fun(a0)(a1)(a2);}
function F4(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return fun(a0,a1,a2,a3);};};};};wrapper.a=4;wrapper.f=fun;return wrapper;}
function A4(fun,a0,a1,a2,a3){return fun.a===4?fun.f(a0,a1,a2,a3):fun(a0)(a1)(a2)(a3);}
function F5(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return function(a4){return fun(a0,a1,a2,a3,a4);};};};};};wrapper.a=5;wrapper.f=fun;return wrapper;}
function A5(fun,a0,a1,a2,a3,a4){return fun.a===5?fun.f(a0,a1,a2,a3,a4):fun(a0)(a1)(a2)(a3)(a4);}
function F6(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return function(a4){return function(a5){return fun(a0,a1,a2,a3,a4,a5);};};};};};};wrapper.a=6;wrapper.f=fun;return wrapper;}
function A6(fun,a0,a1,a2,a3,a4,a5){return fun.a===6?fun.f(a0,a1,a2,a3,a4,a5):fun(a0)(a1)(a2)(a3)(a4)(a5);}
function F7(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return function(a4){return function(a5){return function(a6){return fun(a0,a1,a2,a3,a4,a5,a6);};};};};};};};wrapper.a=7;wrapper.f=fun;return wrapper;}
function A7(fun,a0,a1,a2,a3,a4,a5,a6){return fun.a===7?fun.f(a0,a1,a2,a3,a4,a5,a6):fun(a0)(a1)(a2)(a3)(a4)(a5)(a6);}
function F8(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return function(a4){return function(a5){return function(a6){return function(a7){return fun(a0,a1,a2,a3,a4,a5,a6,a7);};};};};};};};};wrapper.a=8;wrapper.f=fun;return wrapper;}
function A8(fun,a0,a1,a2,a3,a4,a5,a6,a7){return fun.a===8?fun.f(a0,a1,a2,a3,a4,a5,a6,a7):fun(a0)(a1)(a2)(a3)(a4)(a5)(a6)(a7);}
function F9(fun){var wrapper=function(a0){return function(a1){return function(a2){return function(a3){return function(a4){return function(a5){return function(a6){return function(a7){return function(a8){return fun(a0,a1,a2,a3,a4,a5,a6,a7,a8);};};};};};};};};};wrapper.a=9;wrapper.f=fun;return wrapper;}
function A9(fun,a0,a1,a2,a3,a4,a5,a6,a7,a8){return fun.a===9?fun.f(a0,a1,a2,a3,a4,a5,a6,a7,a8):fun(a0)(a1)(a2)(a3)(a4)(a5)(a6)(a7)(a8);}



// MATH

var _Basics_add = F2(function(a, b) { return a + b; });
var _Basics_sub = F2(function(a, b) { return a - b; });
var _Basics_mul = F2(function(a, b) { return a * b; });
var _Basics_fdiv = F2(function(a, b) { return a / b; });
var _Basics_idiv = F2(function(a, b) { return (a / b) | 0; });
var _Basics_pow = F2(Math.pow);

var _Basics_remainderBy = F2(function(b, a) { return a % b; });

// https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/divmodnote-letter.pdf
var _Basics_modBy = F2(function(modulus, x)
{
	var answer = x % modulus;
	return modulus === 0
		? _Debug_crash(11)
		:
	((answer > 0 && modulus < 0) || (answer < 0 && modulus > 0))
		? answer + modulus
		: answer;
});


// TRIGONOMETRY

var _Basics_pi = Math.PI;
var _Basics_e = Math.E;
var _Basics_cos = Math.cos;
var _Basics_sin = Math.sin;
var _Basics_tan = Math.tan;
var _Basics_acos = Math.acos;
var _Basics_asin = Math.asin;
var _Basics_atan = Math.atan;
var _Basics_atan2 = F2(Math.atan2);


// MORE MATH

function _Basics_toFloat(x) { return x; }
function _Basics_truncate(n) { return n | 0; }
function _Basics_isInfinite(n) { return n === Infinity || n === -Infinity; }

var _Basics_ceiling = Math.ceil;
var _Basics_floor = Math.floor;
var _Basics_round = Math.round;
var _Basics_sqrt = Math.sqrt;
var _Basics_log = Math.log;
var _Basics_isNaN = isNaN;


// BOOLEANS

function _Basics_not(bool) { return !bool; }
var _Basics_and = F2(function(a, b) { return a && b; });
var _Basics_or  = F2(function(a, b) { return a || b; });
var _Basics_xor = F2(function(a, b) { return a !== b; });




// EQUALITY

function _Utils_eq(x, y)
{
	for (
		var pair, stack = [], isEqual = _Utils_eqHelp(x, y, 0, stack);
		isEqual && (pair = stack.pop());
		isEqual = _Utils_eqHelp(pair.a, pair.b, 0, stack)
		)
	{}

	return isEqual;
}

function _Utils_eqHelp(x, y, depth, stack)
{
	if (depth > 100)
	{
		stack.push(_Utils_Tuple2(x,y));
		return true;
	}

	if (x === y)
	{
		return true;
	}

	if (typeof x !== 'object' || x === null || y === null)
	{
		typeof x === 'function' && _Debug_crash(5);
		return false;
	}

	/**_UNUSED/
	if (x.$ === 'Set_elm_builtin')
	{
		x = $elm$core$Set$toList(x);
		y = $elm$core$Set$toList(y);
	}
	if (x.$ === 'RBNode_elm_builtin' || x.$ === 'RBEmpty_elm_builtin')
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	/**/
	if (x.$ < 0)
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	for (var key in x)
	{
		if (!_Utils_eqHelp(x[key], y[key], depth + 1, stack))
		{
			return false;
		}
	}
	return true;
}

var _Utils_equal = F2(_Utils_eq);
var _Utils_notEqual = F2(function(a, b) { return !_Utils_eq(a,b); });



// COMPARISONS

// Code in Generate/JavaScript.hs, Basics.js, and List.js depends on
// the particular integer values assigned to LT, EQ, and GT.

function _Utils_cmp(x, y, ord)
{
	if (typeof x !== 'object')
	{
		return x === y ? /*EQ*/ 0 : x < y ? /*LT*/ -1 : /*GT*/ 1;
	}

	/**_UNUSED/
	if (x instanceof String)
	{
		var a = x.valueOf();
		var b = y.valueOf();
		return a === b ? 0 : a < b ? -1 : 1;
	}
	//*/

	/**/
	if (typeof x.$ === 'undefined')
	//*/
	/**_UNUSED/
	if (x.$[0] === '#')
	//*/
	{
		return (ord = _Utils_cmp(x.a, y.a))
			? ord
			: (ord = _Utils_cmp(x.b, y.b))
				? ord
				: _Utils_cmp(x.c, y.c);
	}

	// traverse conses until end of a list or a mismatch
	for (; x.b && y.b && !(ord = _Utils_cmp(x.a, y.a)); x = x.b, y = y.b) {} // WHILE_CONSES
	return ord || (x.b ? /*GT*/ 1 : y.b ? /*LT*/ -1 : /*EQ*/ 0);
}

var _Utils_lt = F2(function(a, b) { return _Utils_cmp(a, b) < 0; });
var _Utils_le = F2(function(a, b) { return _Utils_cmp(a, b) < 1; });
var _Utils_gt = F2(function(a, b) { return _Utils_cmp(a, b) > 0; });
var _Utils_ge = F2(function(a, b) { return _Utils_cmp(a, b) >= 0; });

var _Utils_compare = F2(function(x, y)
{
	var n = _Utils_cmp(x, y);
	return n < 0 ? $elm$core$Basics$LT : n ? $elm$core$Basics$GT : $elm$core$Basics$EQ;
});


// COMMON VALUES

var _Utils_Tuple0 = 0;
var _Utils_Tuple0_UNUSED = { $: '#0' };

function _Utils_Tuple2(a, b) { return { a: a, b: b }; }
function _Utils_Tuple2_UNUSED(a, b) { return { $: '#2', a: a, b: b }; }

function _Utils_Tuple3(a, b, c) { return { a: a, b: b, c: c }; }
function _Utils_Tuple3_UNUSED(a, b, c) { return { $: '#3', a: a, b: b, c: c }; }

function _Utils_chr(c) { return c; }
function _Utils_chr_UNUSED(c) { return new String(c); }


// RECORDS

function _Utils_update(oldRecord, updatedFields)
{
	var newRecord = {};

	for (var key in oldRecord)
	{
		newRecord[key] = oldRecord[key];
	}

	for (var key in updatedFields)
	{
		newRecord[key] = updatedFields[key];
	}

	return newRecord;
}


// APPEND

var _Utils_append = F2(_Utils_ap);

function _Utils_ap(xs, ys)
{
	// append Strings
	if (typeof xs === 'string')
	{
		return xs + ys;
	}

	// append Lists
	if (!xs.b)
	{
		return ys;
	}
	var root = _List_Cons(xs.a, ys);
	xs = xs.b
	for (var curr = root; xs.b; xs = xs.b) // WHILE_CONS
	{
		curr = curr.b = _List_Cons(xs.a, ys);
	}
	return root;
}




var _Bitwise_and = F2(function(a, b)
{
	return a & b;
});

var _Bitwise_or = F2(function(a, b)
{
	return a | b;
});

var _Bitwise_xor = F2(function(a, b)
{
	return a ^ b;
});

function _Bitwise_complement(a)
{
	return ~a;
};

var _Bitwise_shiftLeftBy = F2(function(offset, a)
{
	return a << offset;
});

var _Bitwise_shiftRightBy = F2(function(offset, a)
{
	return a >> offset;
});

var _Bitwise_shiftRightZfBy = F2(function(offset, a)
{
	return a >>> offset;
});




var _JsArray_empty = [];

function _JsArray_singleton(value)
{
    return [value];
}

function _JsArray_length(array)
{
    return array.length;
}

var _JsArray_initialize = F3(function(size, offset, func)
{
    var result = new Array(size);

    for (var i = 0; i < size; i++)
    {
        result[i] = func(offset + i);
    }

    return result;
});

var _JsArray_initializeFromList = F2(function (max, ls)
{
    var result = new Array(max);

    for (var i = 0; i < max && ls.b; i++)
    {
        result[i] = ls.a;
        ls = ls.b;
    }

    result.length = i;
    return _Utils_Tuple2(result, ls);
});

var _JsArray_unsafeGet = F2(function(index, array)
{
    return array[index];
});

var _JsArray_unsafeSet = F3(function(index, value, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[index] = value;
    return result;
});

var _JsArray_push = F2(function(value, array)
{
    var length = array.length;
    var result = new Array(length + 1);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[length] = value;
    return result;
});

var _JsArray_foldl = F3(function(func, acc, array)
{
    var length = array.length;

    for (var i = 0; i < length; i++)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_foldr = F3(function(func, acc, array)
{
    for (var i = array.length - 1; i >= 0; i--)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_map = F2(function(func, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = func(array[i]);
    }

    return result;
});

var _JsArray_indexedMap = F3(function(func, offset, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = A2(func, offset + i, array[i]);
    }

    return result;
});

var _JsArray_slice = F3(function(from, to, array)
{
    return array.slice(from, to);
});

var _JsArray_appendN = F3(function(n, dest, source)
{
    var destLen = dest.length;
    var itemsToCopy = n - destLen;

    if (itemsToCopy > source.length)
    {
        itemsToCopy = source.length;
    }

    var size = destLen + itemsToCopy;
    var result = new Array(size);

    for (var i = 0; i < destLen; i++)
    {
        result[i] = dest[i];
    }

    for (var i = 0; i < itemsToCopy; i++)
    {
        result[i + destLen] = source[i];
    }

    return result;
});




var _List_Nil = { $: 0 };
var _List_Nil_UNUSED = { $: '[]' };

function _List_Cons(hd, tl) { return { $: 1, a: hd, b: tl }; }
function _List_Cons_UNUSED(hd, tl) { return { $: '::', a: hd, b: tl }; }


var _List_cons = F2(_List_Cons);

function _List_fromArray(arr)
{
	var out = _List_Nil;
	for (var i = arr.length; i--; )
	{
		out = _List_Cons(arr[i], out);
	}
	return out;
}

function _List_toArray(xs)
{
	for (var out = []; xs.b; xs = xs.b) // WHILE_CONS
	{
		out.push(xs.a);
	}
	return out;
}

var _List_map2 = F3(function(f, xs, ys)
{
	for (var arr = []; xs.b && ys.b; xs = xs.b, ys = ys.b) // WHILE_CONSES
	{
		arr.push(A2(f, xs.a, ys.a));
	}
	return _List_fromArray(arr);
});

var _List_map3 = F4(function(f, xs, ys, zs)
{
	for (var arr = []; xs.b && ys.b && zs.b; xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A3(f, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map4 = F5(function(f, ws, xs, ys, zs)
{
	for (var arr = []; ws.b && xs.b && ys.b && zs.b; ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A4(f, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map5 = F6(function(f, vs, ws, xs, ys, zs)
{
	for (var arr = []; vs.b && ws.b && xs.b && ys.b && zs.b; vs = vs.b, ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A5(f, vs.a, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_sortBy = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		return _Utils_cmp(f(a), f(b));
	}));
});

var _List_sortWith = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		var ord = A2(f, a, b);
		return ord === $elm$core$Basics$EQ ? 0 : ord === $elm$core$Basics$LT ? -1 : 1;
	}));
});




function _Char_toCode(char)
{
	var code = char.charCodeAt(0);
	if (0xD800 <= code && code <= 0xDBFF)
	{
		return (code - 0xD800) * 0x400 + char.charCodeAt(1) - 0xDC00 + 0x10000
	}
	return code;
}

function _Char_fromCode(code)
{
	return _Utils_chr(
		(code < 0 || 0x10FFFF < code)
			? '\uFFFD'
			:
		(code <= 0xFFFF)
			? String.fromCharCode(code)
			:
		(code -= 0x10000,
			String.fromCharCode(Math.floor(code / 0x400) + 0xD800, code % 0x400 + 0xDC00)
		)
	);
}

function _Char_toUpper(char)
{
	return _Utils_chr(char.toUpperCase());
}

function _Char_toLower(char)
{
	return _Utils_chr(char.toLowerCase());
}

function _Char_toLocaleUpper(char)
{
	return _Utils_chr(char.toLocaleUpperCase());
}

function _Char_toLocaleLower(char)
{
	return _Utils_chr(char.toLocaleLowerCase());
}




// LOG

var _Debug_log = F2(function(tag, value)
{
	return value;
});

var _Debug_log_UNUSED = F2(function(tag, value)
{
	console.log(tag + ': ' + _Debug_toString(value));
	return value;
});


// TODOS

function _Debug_todo(moduleName, region)
{
	return function(message) {
		_Debug_crash(8, moduleName, region, message);
	};
}

function _Debug_todoCase(moduleName, region, value)
{
	return function(message) {
		_Debug_crash(9, moduleName, region, value, message);
	};
}


// TO STRING

function _Debug_toString(value)
{
	return '<internals>';
}

function _Debug_toString_UNUSED(value)
{
	return _Debug_toAnsiString(false, value);
}

function _Debug_toAnsiString(ansi, value)
{
	if (typeof value === 'function')
	{
		return _Debug_internalColor(ansi, '<function>');
	}

	if (typeof value === 'boolean')
	{
		return _Debug_ctorColor(ansi, value ? 'True' : 'False');
	}

	if (typeof value === 'number')
	{
		return _Debug_numberColor(ansi, value + '');
	}

	if (value instanceof String)
	{
		return _Debug_charColor(ansi, "'" + _Debug_addSlashes(value, true) + "'");
	}

	if (typeof value === 'string')
	{
		return _Debug_stringColor(ansi, '"' + _Debug_addSlashes(value, false) + '"');
	}

	if (typeof value === 'object' && '$' in value)
	{
		var tag = value.$;

		if (typeof tag === 'number')
		{
			return _Debug_internalColor(ansi, '<internals>');
		}

		if (tag[0] === '#')
		{
			var output = [];
			for (var k in value)
			{
				if (k === '$') continue;
				output.push(_Debug_toAnsiString(ansi, value[k]));
			}
			return '(' + output.join(',') + ')';
		}

		if (tag === 'Set_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Set')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Set$toList(value));
		}

		if (tag === 'RBNode_elm_builtin' || tag === 'RBEmpty_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Dict')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Dict$toList(value));
		}

		if (tag === 'Array_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Array')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Array$toList(value));
		}

		if (tag === '::' || tag === '[]')
		{
			var output = '[';

			value.b && (output += _Debug_toAnsiString(ansi, value.a), value = value.b)

			for (; value.b; value = value.b) // WHILE_CONS
			{
				output += ',' + _Debug_toAnsiString(ansi, value.a);
			}
			return output + ']';
		}

		var output = '';
		for (var i in value)
		{
			if (i === '$') continue;
			var str = _Debug_toAnsiString(ansi, value[i]);
			var c0 = str[0];
			var parenless = c0 === '{' || c0 === '(' || c0 === '[' || c0 === '<' || c0 === '"' || str.indexOf(' ') < 0;
			output += ' ' + (parenless ? str : '(' + str + ')');
		}
		return _Debug_ctorColor(ansi, tag) + output;
	}

	if (typeof DataView === 'function' && value instanceof DataView)
	{
		return _Debug_stringColor(ansi, '<' + value.byteLength + ' bytes>');
	}

	if (typeof File === 'function' && value instanceof File)
	{
		return _Debug_internalColor(ansi, '<' + value.name + '>');
	}

	if (typeof value === 'object')
	{
		var output = [];
		for (var key in value)
		{
			var field = key[0] === '_' ? key.slice(1) : key;
			output.push(_Debug_fadeColor(ansi, field) + ' = ' + _Debug_toAnsiString(ansi, value[key]));
		}
		if (output.length === 0)
		{
			return '{}';
		}
		return '{ ' + output.join(', ') + ' }';
	}

	return _Debug_internalColor(ansi, '<internals>');
}

function _Debug_addSlashes(str, isChar)
{
	var s = str
		.replace(/\\/g, '\\\\')
		.replace(/\n/g, '\\n')
		.replace(/\t/g, '\\t')
		.replace(/\r/g, '\\r')
		.replace(/\v/g, '\\v')
		.replace(/\0/g, '\\0');

	if (isChar)
	{
		return s.replace(/\'/g, '\\\'');
	}
	else
	{
		return s.replace(/\"/g, '\\"');
	}
}

function _Debug_ctorColor(ansi, string)
{
	return ansi ? '\x1b[96m' + string + '\x1b[0m' : string;
}

function _Debug_numberColor(ansi, string)
{
	return ansi ? '\x1b[95m' + string + '\x1b[0m' : string;
}

function _Debug_stringColor(ansi, string)
{
	return ansi ? '\x1b[93m' + string + '\x1b[0m' : string;
}

function _Debug_charColor(ansi, string)
{
	return ansi ? '\x1b[92m' + string + '\x1b[0m' : string;
}

function _Debug_fadeColor(ansi, string)
{
	return ansi ? '\x1b[37m' + string + '\x1b[0m' : string;
}

function _Debug_internalColor(ansi, string)
{
	return ansi ? '\x1b[94m' + string + '\x1b[0m' : string;
}

function _Debug_toHexDigit(n)
{
	return String.fromCharCode(n < 10 ? 48 + n : 55 + n);
}


// CRASH


function _Debug_crash(identifier)
{
	throw new Error('https://github.com/elm/core/blob/1.0.0/hints/' + identifier + '.md');
}


function _Debug_crash_UNUSED(identifier, fact1, fact2, fact3, fact4)
{
	switch(identifier)
	{
		case 0:
			throw new Error('What node should I take over? In JavaScript I need something like:\n\n    Elm.Main.init({\n        node: document.getElementById("elm-node")\n    })\n\nYou need to do this with any Browser.sandbox or Browser.element program.');

		case 1:
			throw new Error('Browser.application programs cannot handle URLs like this:\n\n    ' + document.location.href + '\n\nWhat is the root? The root of your file system? Try looking at this program with `elm reactor` or some other server.');

		case 2:
			var jsonErrorString = fact1;
			throw new Error('Problem with the flags given to your Elm program on initialization.\n\n' + jsonErrorString);

		case 3:
			var portName = fact1;
			throw new Error('There can only be one port named `' + portName + '`, but your program has multiple.');

		case 4:
			var portName = fact1;
			var problem = fact2;
			throw new Error('Trying to send an unexpected type of value through port `' + portName + '`:\n' + problem);

		case 5:
			throw new Error('Trying to use `(==)` on functions.\nThere is no way to know if functions are "the same" in the Elm sense.\nRead more about this at https://package.elm-lang.org/packages/elm/core/latest/Basics#== which describes why it is this way and what the better version will look like.');

		case 6:
			var moduleName = fact1;
			throw new Error('Your page is loading multiple Elm scripts with a module named ' + moduleName + '. Maybe a duplicate script is getting loaded accidentally? If not, rename one of them so I know which is which!');

		case 8:
			var moduleName = fact1;
			var region = fact2;
			var message = fact3;
			throw new Error('TODO in module `' + moduleName + '` ' + _Debug_regionToString(region) + '\n\n' + message);

		case 9:
			var moduleName = fact1;
			var region = fact2;
			var value = fact3;
			var message = fact4;
			throw new Error(
				'TODO in module `' + moduleName + '` from the `case` expression '
				+ _Debug_regionToString(region) + '\n\nIt received the following value:\n\n    '
				+ _Debug_toString(value).replace('\n', '\n    ')
				+ '\n\nBut the branch that handles it says:\n\n    ' + message.replace('\n', '\n    ')
			);

		case 10:
			throw new Error('Bug in https://github.com/elm/virtual-dom/issues');

		case 11:
			throw new Error('Cannot perform mod 0. Division by zero error.');
	}
}

function _Debug_regionToString(region)
{
	if (region.aa.C === region.af.C)
	{
		return 'on line ' + region.aa.C;
	}
	return 'on lines ' + region.aa.C + ' through ' + region.af.C;
}




var _String_cons = F2(function(chr, str)
{
	return chr + str;
});

function _String_uncons(string)
{
	var word = string.charCodeAt(0);
	return word
		? $elm$core$Maybe$Just(
			0xD800 <= word && word <= 0xDBFF
				? _Utils_Tuple2(_Utils_chr(string[0] + string[1]), string.slice(2))
				: _Utils_Tuple2(_Utils_chr(string[0]), string.slice(1))
		)
		: $elm$core$Maybe$Nothing;
}

var _String_append = F2(function(a, b)
{
	return a + b;
});

function _String_length(str)
{
	return str.length;
}

var _String_map = F2(function(func, string)
{
	var len = string.length;
	var array = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = string.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			array[i] = func(_Utils_chr(string[i] + string[i+1]));
			i += 2;
			continue;
		}
		array[i] = func(_Utils_chr(string[i]));
		i++;
	}
	return array.join('');
});

var _String_filter = F2(function(isGood, str)
{
	var arr = [];
	var len = str.length;
	var i = 0;
	while (i < len)
	{
		var char = str[i];
		var word = str.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += str[i];
			i++;
		}

		if (isGood(_Utils_chr(char)))
		{
			arr.push(char);
		}
	}
	return arr.join('');
});

function _String_reverse(str)
{
	var len = str.length;
	var arr = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = str.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			arr[len - i] = str[i + 1];
			i++;
			arr[len - i] = str[i - 1];
			i++;
		}
		else
		{
			arr[len - i] = str[i];
			i++;
		}
	}
	return arr.join('');
}

var _String_foldl = F3(function(func, state, string)
{
	var len = string.length;
	var i = 0;
	while (i < len)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += string[i];
			i++;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_foldr = F3(function(func, state, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_split = F2(function(sep, str)
{
	return str.split(sep);
});

var _String_join = F2(function(sep, strs)
{
	return strs.join(sep);
});

var _String_slice = F3(function(start, end, str) {
	return str.slice(start, end);
});

function _String_trim(str)
{
	return str.trim();
}

function _String_trimLeft(str)
{
	return str.replace(/^\s+/, '');
}

function _String_trimRight(str)
{
	return str.replace(/\s+$/, '');
}

function _String_words(str)
{
	return _List_fromArray(str.trim().split(/\s+/g));
}

function _String_lines(str)
{
	return _List_fromArray(str.split(/\r\n|\r|\n/g));
}

function _String_toUpper(str)
{
	return str.toUpperCase();
}

function _String_toLower(str)
{
	return str.toLowerCase();
}

var _String_any = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (isGood(_Utils_chr(char)))
		{
			return true;
		}
	}
	return false;
});

var _String_all = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (!isGood(_Utils_chr(char)))
		{
			return false;
		}
	}
	return true;
});

var _String_contains = F2(function(sub, str)
{
	return str.indexOf(sub) > -1;
});

var _String_startsWith = F2(function(sub, str)
{
	return str.indexOf(sub) === 0;
});

var _String_endsWith = F2(function(sub, str)
{
	return str.length >= sub.length &&
		str.lastIndexOf(sub) === str.length - sub.length;
});

var _String_indexes = F2(function(sub, str)
{
	var subLen = sub.length;

	if (subLen < 1)
	{
		return _List_Nil;
	}

	var i = 0;
	var is = [];

	while ((i = str.indexOf(sub, i)) > -1)
	{
		is.push(i);
		i = i + subLen;
	}

	return _List_fromArray(is);
});


// TO STRING

function _String_fromNumber(number)
{
	return number + '';
}


// INT CONVERSIONS

function _String_toInt(str)
{
	var total = 0;
	var code0 = str.charCodeAt(0);
	var start = code0 == 0x2B /* + */ || code0 == 0x2D /* - */ ? 1 : 0;

	for (var i = start; i < str.length; ++i)
	{
		var code = str.charCodeAt(i);
		if (code < 0x30 || 0x39 < code)
		{
			return $elm$core$Maybe$Nothing;
		}
		total = 10 * total + code - 0x30;
	}

	return i == start
		? $elm$core$Maybe$Nothing
		: $elm$core$Maybe$Just(code0 == 0x2D ? -total : total);
}


// FLOAT CONVERSIONS

function _String_toFloat(s)
{
	// check if it is a hex, octal, or binary number
	if (s.length === 0 || /[\sxbo]/.test(s))
	{
		return $elm$core$Maybe$Nothing;
	}
	var n = +s;
	// faster isNaN check
	return n === n ? $elm$core$Maybe$Just(n) : $elm$core$Maybe$Nothing;
}

function _String_fromList(chars)
{
	return _List_toArray(chars).join('');
}






// PROGRAMS


var _Platform_worker = F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.f,
		impl.h,
		impl.j,
		function() { return function() {} }
	);
});



// INITIALIZE A PROGRAM


function _Platform_initialize(flagDecoder, args, init, update, subscriptions, stepperBuilder)
{
	var result = A2(_Json_run, flagDecoder, _Json_wrap(args ? args['flags'] : undefined));
	$elm$core$Result$isOk(result) || _Debug_crash(2 /**_UNUSED/, _Json_errorToString(result.a) /**/);
	var managers = {};
	result = init(result.a);
	var model = result.a;
	var stepper = stepperBuilder(sendToApp, model);
	var ports = _Platform_setupEffects(managers, sendToApp);

	function sendToApp(msg, viewMetadata)
	{
		result = A2(update, msg, model);
		stepper(model = result.a, viewMetadata);
		_Platform_dispatchEffects(managers, result.b, subscriptions(model));
	}

	_Platform_dispatchEffects(managers, result.b, subscriptions(model));

	return ports ? { ports: ports } : {};
}



// TRACK PRELOADS
//
// This is used by code in elm/browser and elm/http
// to register any HTTP requests that are triggered by init.
//


var _Platform_preload;


function _Platform_registerPreload(url)
{
	_Platform_preload.add(url);
}



// EFFECT MANAGERS


var _Platform_effectManagers = {};


function _Platform_setupEffects(managers, sendToApp)
{
	var ports;

	// setup all necessary effect managers
	for (var key in _Platform_effectManagers)
	{
		var manager = _Platform_effectManagers[key];

		if (manager.a)
		{
			ports = ports || {};
			ports[key] = manager.a(key, sendToApp);
		}

		managers[key] = _Platform_instantiateManager(manager, sendToApp);
	}

	return ports;
}


function _Platform_createManager(init, onEffects, onSelfMsg, cmdMap, subMap)
{
	return {
		b: init,
		c: onEffects,
		d: onSelfMsg,
		e: cmdMap,
		f: subMap
	};
}


function _Platform_instantiateManager(info, sendToApp)
{
	var router = {
		g: sendToApp,
		h: undefined
	};

	var onEffects = info.c;
	var onSelfMsg = info.d;
	var cmdMap = info.e;
	var subMap = info.f;

	function loop(state)
	{
		return A2(_Scheduler_andThen, loop, _Scheduler_receive(function(msg)
		{
			var value = msg.a;

			if (msg.$ === 0)
			{
				return A3(onSelfMsg, router, value, state);
			}

			return cmdMap && subMap
				? A4(onEffects, router, value.i, value.j, state)
				: A3(onEffects, router, cmdMap ? value.i : value.j, state);
		}));
	}

	return router.h = _Scheduler_rawSpawn(A2(_Scheduler_andThen, loop, info.b));
}



// ROUTING


var _Platform_sendToApp = F2(function(router, msg)
{
	return _Scheduler_binding(function(callback)
	{
		router.g(msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});


var _Platform_sendToSelf = F2(function(router, msg)
{
	return A2(_Scheduler_send, router.h, {
		$: 0,
		a: msg
	});
});



// BAGS


function _Platform_leaf(home)
{
	return function(value)
	{
		return {
			$: 1,
			k: home,
			l: value
		};
	};
}


function _Platform_batch(list)
{
	return {
		$: 2,
		m: list
	};
}


var _Platform_map = F2(function(tagger, bag)
{
	return {
		$: 3,
		n: tagger,
		o: bag
	}
});



// PIPE BAGS INTO EFFECT MANAGERS


function _Platform_dispatchEffects(managers, cmdBag, subBag)
{
	var effectsDict = {};
	_Platform_gatherEffects(true, cmdBag, effectsDict, null);
	_Platform_gatherEffects(false, subBag, effectsDict, null);

	for (var home in managers)
	{
		_Scheduler_rawSend(managers[home], {
			$: 'fx',
			a: effectsDict[home] || { i: _List_Nil, j: _List_Nil }
		});
	}
}


function _Platform_gatherEffects(isCmd, bag, effectsDict, taggers)
{
	switch (bag.$)
	{
		case 1:
			var home = bag.k;
			var effect = _Platform_toEffect(isCmd, home, taggers, bag.l);
			effectsDict[home] = _Platform_insert(isCmd, effect, effectsDict[home]);
			return;

		case 2:
			for (var list = bag.m; list.b; list = list.b) // WHILE_CONS
			{
				_Platform_gatherEffects(isCmd, list.a, effectsDict, taggers);
			}
			return;

		case 3:
			_Platform_gatherEffects(isCmd, bag.o, effectsDict, {
				p: bag.n,
				q: taggers
			});
			return;
	}
}


function _Platform_toEffect(isCmd, home, taggers, value)
{
	function applyTaggers(x)
	{
		for (var temp = taggers; temp; temp = temp.q)
		{
			x = temp.p(x);
		}
		return x;
	}

	var map = isCmd
		? _Platform_effectManagers[home].e
		: _Platform_effectManagers[home].f;

	return A2(map, applyTaggers, value)
}


function _Platform_insert(isCmd, newEffect, effects)
{
	effects = effects || { i: _List_Nil, j: _List_Nil };

	isCmd
		? (effects.i = _List_Cons(newEffect, effects.i))
		: (effects.j = _List_Cons(newEffect, effects.j));

	return effects;
}



// PORTS


function _Platform_checkPortName(name)
{
	if (_Platform_effectManagers[name])
	{
		_Debug_crash(3, name)
	}
}



// OUTGOING PORTS


function _Platform_outgoingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		e: _Platform_outgoingPortMap,
		r: converter,
		a: _Platform_setupOutgoingPort
	};
	return _Platform_leaf(name);
}


var _Platform_outgoingPortMap = F2(function(tagger, value) { return value; });


function _Platform_setupOutgoingPort(name)
{
	var subs = [];
	var converter = _Platform_effectManagers[name].r;

	// CREATE MANAGER

	var init = _Process_sleep(0);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, cmdList, state)
	{
		for ( ; cmdList.b; cmdList = cmdList.b) // WHILE_CONS
		{
			// grab a separate reference to subs in case unsubscribe is called
			var currentSubs = subs;
			var value = _Json_unwrap(converter(cmdList.a));
			for (var i = 0; i < currentSubs.length; i++)
			{
				currentSubs[i](value);
			}
		}
		return init;
	});

	// PUBLIC API

	function subscribe(callback)
	{
		subs.push(callback);
	}

	function unsubscribe(callback)
	{
		// copy subs into a new array in case unsubscribe is called within a
		// subscribed callback
		subs = subs.slice();
		var index = subs.indexOf(callback);
		if (index >= 0)
		{
			subs.splice(index, 1);
		}
	}

	return {
		subscribe: subscribe,
		unsubscribe: unsubscribe
	};
}



// INCOMING PORTS


function _Platform_incomingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		f: _Platform_incomingPortMap,
		r: converter,
		a: _Platform_setupIncomingPort
	};
	return _Platform_leaf(name);
}


var _Platform_incomingPortMap = F2(function(tagger, finalTagger)
{
	return function(value)
	{
		return tagger(finalTagger(value));
	};
});


function _Platform_setupIncomingPort(name, sendToApp)
{
	var subs = _List_Nil;
	var converter = _Platform_effectManagers[name].r;

	// CREATE MANAGER

	var init = _Scheduler_succeed(null);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, subList, state)
	{
		subs = subList;
		return init;
	});

	// PUBLIC API

	function send(incomingValue)
	{
		var result = A2(_Json_run, converter, _Json_wrap(incomingValue));

		$elm$core$Result$isOk(result) || _Debug_crash(4, name, result.a);

		var value = result.a;
		for (var temp = subs; temp.b; temp = temp.b) // WHILE_CONS
		{
			sendToApp(temp.a(value));
		}
	}

	return { send: send };
}



// EXPORT ELM MODULES
//
// Have DEBUG and PROD versions so that we can (1) give nicer errors in
// debug mode and (2) not pay for the bits needed for that in prod mode.
//


function _Platform_export(exports)
{
	scope['Elm']
		? _Platform_mergeExportsProd(scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsProd(obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6)
				: _Platform_mergeExportsProd(obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}


function _Platform_export_UNUSED(exports)
{
	scope['Elm']
		? _Platform_mergeExportsDebug('Elm', scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsDebug(moduleName, obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6, moduleName)
				: _Platform_mergeExportsDebug(moduleName + '.' + name, obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}




// TASKS

function _Scheduler_succeed(value)
{
	return {
		$: 0,
		a: value
	};
}

function _Scheduler_fail(error)
{
	return {
		$: 1,
		a: error
	};
}

function _Scheduler_binding(callback)
{
	return {
		$: 2,
		b: callback,
		c: null
	};
}

var _Scheduler_andThen = F2(function(callback, task)
{
	return {
		$: 3,
		b: callback,
		d: task
	};
});

var _Scheduler_onError = F2(function(callback, task)
{
	return {
		$: 4,
		b: callback,
		d: task
	};
});

function _Scheduler_receive(callback)
{
	return {
		$: 5,
		b: callback
	};
}


// PROCESSES

var _Scheduler_guid = 0;

function _Scheduler_rawSpawn(task)
{
	var proc = {
		$: 0,
		e: _Scheduler_guid++,
		f: task,
		g: null,
		h: []
	};

	_Scheduler_enqueue(proc);

	return proc;
}

function _Scheduler_spawn(task)
{
	return _Scheduler_binding(function(callback) {
		callback(_Scheduler_succeed(_Scheduler_rawSpawn(task)));
	});
}

function _Scheduler_rawSend(proc, msg)
{
	proc.h.push(msg);
	_Scheduler_enqueue(proc);
}

var _Scheduler_send = F2(function(proc, msg)
{
	return _Scheduler_binding(function(callback) {
		_Scheduler_rawSend(proc, msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});

function _Scheduler_kill(proc)
{
	return _Scheduler_binding(function(callback) {
		var task = proc.f;
		if (task.$ === 2 && task.c)
		{
			task.c();
		}

		proc.f = null;

		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
}


/* STEP PROCESSES

type alias Process =
  { $ : tag
  , id : unique_id
  , root : Task
  , stack : null | { $: SUCCEED | FAIL, a: callback, b: stack }
  , mailbox : [msg]
  }

*/


var _Scheduler_working = false;
var _Scheduler_queue = [];


function _Scheduler_enqueue(proc)
{
	_Scheduler_queue.push(proc);
	if (_Scheduler_working)
	{
		return;
	}
	_Scheduler_working = true;
	while (proc = _Scheduler_queue.shift())
	{
		_Scheduler_step(proc);
	}
	_Scheduler_working = false;
}


function _Scheduler_step(proc)
{
	while (proc.f)
	{
		var rootTag = proc.f.$;
		if (rootTag === 0 || rootTag === 1)
		{
			while (proc.g && proc.g.$ !== rootTag)
			{
				proc.g = proc.g.i;
			}
			if (!proc.g)
			{
				return;
			}
			proc.f = proc.g.b(proc.f.a);
			proc.g = proc.g.i;
		}
		else if (rootTag === 2)
		{
			proc.f.c = proc.f.b(function(newRoot) {
				proc.f = newRoot;
				_Scheduler_enqueue(proc);
			});
			return;
		}
		else if (rootTag === 5)
		{
			if (proc.h.length === 0)
			{
				return;
			}
			proc.f = proc.f.b(proc.h.shift());
		}
		else // if (rootTag === 3 || rootTag === 4)
		{
			proc.g = {
				$: rootTag === 3 ? 0 : 1,
				b: proc.f.b,
				i: proc.g
			};
			proc.f = proc.f.d;
		}
	}
}




/**_UNUSED/
function _Json_errorToString(error)
{
	return $elm$json$Json$Decode$errorToString(error);
}
//*/


// CORE DECODERS

function _Json_succeed(msg)
{
	return {
		$: 0,
		a: msg
	};
}

function _Json_fail(msg)
{
	return {
		$: 1,
		a: msg
	};
}

function _Json_decodePrim(decoder)
{
	return { $: 2, b: decoder };
}

var _Json_decodeInt = _Json_decodePrim(function(value) {
	return (typeof value !== 'number')
		? _Json_expecting('an INT', value)
		:
	(-2147483647 < value && value < 2147483647 && (value | 0) === value)
		? $elm$core$Result$Ok(value)
		:
	(isFinite(value) && !(value % 1))
		? $elm$core$Result$Ok(value)
		: _Json_expecting('an INT', value);
});

var _Json_decodeBool = _Json_decodePrim(function(value) {
	return (typeof value === 'boolean')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a BOOL', value);
});

var _Json_decodeFloat = _Json_decodePrim(function(value) {
	return (typeof value === 'number')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a FLOAT', value);
});

var _Json_decodeValue = _Json_decodePrim(function(value) {
	return $elm$core$Result$Ok(_Json_wrap(value));
});

var _Json_decodeString = _Json_decodePrim(function(value) {
	return (typeof value === 'string')
		? $elm$core$Result$Ok(value)
		: (value instanceof String)
			? $elm$core$Result$Ok(value + '')
			: _Json_expecting('a STRING', value);
});

function _Json_decodeList(decoder) { return { $: 3, b: decoder }; }
function _Json_decodeArray(decoder) { return { $: 4, b: decoder }; }

function _Json_decodeNull(value) { return { $: 5, c: value }; }

var _Json_decodeField = F2(function(field, decoder)
{
	return {
		$: 6,
		d: field,
		b: decoder
	};
});

var _Json_decodeIndex = F2(function(index, decoder)
{
	return {
		$: 7,
		e: index,
		b: decoder
	};
});

function _Json_decodeKeyValuePairs(decoder)
{
	return {
		$: 8,
		b: decoder
	};
}

function _Json_mapMany(f, decoders)
{
	return {
		$: 9,
		f: f,
		g: decoders
	};
}

var _Json_andThen = F2(function(callback, decoder)
{
	return {
		$: 10,
		b: decoder,
		h: callback
	};
});

function _Json_oneOf(decoders)
{
	return {
		$: 11,
		g: decoders
	};
}


// DECODING OBJECTS

var _Json_map1 = F2(function(f, d1)
{
	return _Json_mapMany(f, [d1]);
});

var _Json_map2 = F3(function(f, d1, d2)
{
	return _Json_mapMany(f, [d1, d2]);
});

var _Json_map3 = F4(function(f, d1, d2, d3)
{
	return _Json_mapMany(f, [d1, d2, d3]);
});

var _Json_map4 = F5(function(f, d1, d2, d3, d4)
{
	return _Json_mapMany(f, [d1, d2, d3, d4]);
});

var _Json_map5 = F6(function(f, d1, d2, d3, d4, d5)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5]);
});

var _Json_map6 = F7(function(f, d1, d2, d3, d4, d5, d6)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6]);
});

var _Json_map7 = F8(function(f, d1, d2, d3, d4, d5, d6, d7)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7]);
});

var _Json_map8 = F9(function(f, d1, d2, d3, d4, d5, d6, d7, d8)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7, d8]);
});


// DECODE

var _Json_runOnString = F2(function(decoder, string)
{
	try
	{
		var value = JSON.parse(string);
		return _Json_runHelp(decoder, value);
	}
	catch (e)
	{
		return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'This is not valid JSON! ' + e.message, _Json_wrap(string)));
	}
});

var _Json_run = F2(function(decoder, value)
{
	return _Json_runHelp(decoder, _Json_unwrap(value));
});

function _Json_runHelp(decoder, value)
{
	switch (decoder.$)
	{
		case 2:
			return decoder.b(value);

		case 5:
			return (value === null)
				? $elm$core$Result$Ok(decoder.c)
				: _Json_expecting('null', value);

		case 3:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('a LIST', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _List_fromArray);

		case 4:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _Json_toElmArray);

		case 6:
			var field = decoder.d;
			if (typeof value !== 'object' || value === null || !(field in value))
			{
				return _Json_expecting('an OBJECT with a field named `' + field + '`', value);
			}
			var result = _Json_runHelp(decoder.b, value[field]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, field, result.a));

		case 7:
			var index = decoder.e;
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			if (index >= value.length)
			{
				return _Json_expecting('a LONGER array. Need index ' + index + ' but only see ' + value.length + ' entries', value);
			}
			var result = _Json_runHelp(decoder.b, value[index]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, index, result.a));

		case 8:
			if (typeof value !== 'object' || value === null || _Json_isArray(value))
			{
				return _Json_expecting('an OBJECT', value);
			}

			var keyValuePairs = _List_Nil;
			// TODO test perf of Object.keys and switch when support is good enough
			for (var key in value)
			{
				if (value.hasOwnProperty(key))
				{
					var result = _Json_runHelp(decoder.b, value[key]);
					if (!$elm$core$Result$isOk(result))
					{
						return $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, key, result.a));
					}
					keyValuePairs = _List_Cons(_Utils_Tuple2(key, result.a), keyValuePairs);
				}
			}
			return $elm$core$Result$Ok($elm$core$List$reverse(keyValuePairs));

		case 9:
			var answer = decoder.f;
			var decoders = decoder.g;
			for (var i = 0; i < decoders.length; i++)
			{
				var result = _Json_runHelp(decoders[i], value);
				if (!$elm$core$Result$isOk(result))
				{
					return result;
				}
				answer = answer(result.a);
			}
			return $elm$core$Result$Ok(answer);

		case 10:
			var result = _Json_runHelp(decoder.b, value);
			return (!$elm$core$Result$isOk(result))
				? result
				: _Json_runHelp(decoder.h(result.a), value);

		case 11:
			var errors = _List_Nil;
			for (var temp = decoder.g; temp.b; temp = temp.b) // WHILE_CONS
			{
				var result = _Json_runHelp(temp.a, value);
				if ($elm$core$Result$isOk(result))
				{
					return result;
				}
				errors = _List_Cons(result.a, errors);
			}
			return $elm$core$Result$Err($elm$json$Json$Decode$OneOf($elm$core$List$reverse(errors)));

		case 1:
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, decoder.a, _Json_wrap(value)));

		case 0:
			return $elm$core$Result$Ok(decoder.a);
	}
}

function _Json_runArrayDecoder(decoder, value, toElmValue)
{
	var len = value.length;
	var array = new Array(len);
	for (var i = 0; i < len; i++)
	{
		var result = _Json_runHelp(decoder, value[i]);
		if (!$elm$core$Result$isOk(result))
		{
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, i, result.a));
		}
		array[i] = result.a;
	}
	return $elm$core$Result$Ok(toElmValue(array));
}

function _Json_isArray(value)
{
	return Array.isArray(value) || (typeof FileList === 'function' && value instanceof FileList);
}

function _Json_toElmArray(array)
{
	return A2($elm$core$Array$initialize, array.length, function(i) { return array[i]; });
}

function _Json_expecting(type, value)
{
	return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'Expecting ' + type, _Json_wrap(value)));
}


// EQUALITY

function _Json_equality(x, y)
{
	if (x === y)
	{
		return true;
	}

	if (x.$ !== y.$)
	{
		return false;
	}

	switch (x.$)
	{
		case 0:
		case 1:
			return x.a === y.a;

		case 2:
			return x.b === y.b;

		case 5:
			return x.c === y.c;

		case 3:
		case 4:
		case 8:
			return _Json_equality(x.b, y.b);

		case 6:
			return x.d === y.d && _Json_equality(x.b, y.b);

		case 7:
			return x.e === y.e && _Json_equality(x.b, y.b);

		case 9:
			return x.f === y.f && _Json_listEquality(x.g, y.g);

		case 10:
			return x.h === y.h && _Json_equality(x.b, y.b);

		case 11:
			return _Json_listEquality(x.g, y.g);
	}
}

function _Json_listEquality(aDecoders, bDecoders)
{
	var len = aDecoders.length;
	if (len !== bDecoders.length)
	{
		return false;
	}
	for (var i = 0; i < len; i++)
	{
		if (!_Json_equality(aDecoders[i], bDecoders[i]))
		{
			return false;
		}
	}
	return true;
}


// ENCODE

var _Json_encode = F2(function(indentLevel, value)
{
	return JSON.stringify(_Json_unwrap(value), null, indentLevel) + '';
});

function _Json_wrap_UNUSED(value) { return { $: 0, a: value }; }
function _Json_unwrap_UNUSED(value) { return value.a; }

function _Json_wrap(value) { return value; }
function _Json_unwrap(value) { return value; }

function _Json_emptyArray() { return []; }
function _Json_emptyObject() { return {}; }

var _Json_addField = F3(function(key, value, object)
{
	object[key] = _Json_unwrap(value);
	return object;
});

function _Json_addEntry(func)
{
	return F2(function(entry, array)
	{
		array.push(_Json_unwrap(func(entry)));
		return array;
	});
}

var _Json_encodeNull = _Json_wrap(null);





// ELEMENT


var _Debugger_element;

var _Browser_element = _Debugger_element || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.f,
		impl.h,
		impl.j,
		function(sendToApp, initialModel) {
			var view = impl.k;
			/**/
			var domNode = args['node'];
			//*/
			/**_UNUSED/
			var domNode = args && args['node'] ? args['node'] : _Debug_crash(0);
			//*/
			var currNode = _VirtualDom_virtualize(domNode);

			return _Browser_makeAnimator(initialModel, function(model)
			{
				var nextNode = view(model);
				var patches = _VirtualDom_diff(currNode, nextNode);
				domNode = _VirtualDom_applyPatches(domNode, currNode, patches, sendToApp);
				currNode = nextNode;
			});
		}
	);
});



// DOCUMENT


var _Debugger_document;

var _Browser_document = _Debugger_document || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.f,
		impl.h,
		impl.j,
		function(sendToApp, initialModel) {
			var divertHrefToApp = impl.Q && impl.Q(sendToApp)
			var view = impl.k;
			var title = _VirtualDom_doc.title;
			var bodyNode = _VirtualDom_doc.body;
			var currNode = _VirtualDom_virtualize(bodyNode);
			return _Browser_makeAnimator(initialModel, function(model)
			{
				_VirtualDom_divertHrefToApp = divertHrefToApp;
				var doc = view(model);
				var nextNode = _VirtualDom_node('body')(_List_Nil)(doc.v);
				var patches = _VirtualDom_diff(currNode, nextNode);
				bodyNode = _VirtualDom_applyPatches(bodyNode, currNode, patches, sendToApp);
				currNode = nextNode;
				_VirtualDom_divertHrefToApp = 0;
				(title !== doc.a) && (_VirtualDom_doc.title = title = doc.a);
			});
		}
	);
});



// ANIMATION


var _Browser_cancelAnimationFrame =
	typeof cancelAnimationFrame !== 'undefined'
		? cancelAnimationFrame
		: function(id) { clearTimeout(id); };

var _Browser_requestAnimationFrame =
	typeof requestAnimationFrame !== 'undefined'
		? requestAnimationFrame
		: function(callback) { return setTimeout(callback, 1000 / 60); };


function _Browser_makeAnimator(model, draw)
{
	draw(model);

	var state = 0;

	function updateIfNeeded()
	{
		state = state === 1
			? 0
			: ( _Browser_requestAnimationFrame(updateIfNeeded), draw(model), 1 );
	}

	return function(nextModel, isSync)
	{
		model = nextModel;

		isSync
			? ( draw(model),
				state === 2 && (state = 1)
				)
			: ( state === 0 && _Browser_requestAnimationFrame(updateIfNeeded),
				state = 2
				);
	};
}



// APPLICATION


function _Browser_application(impl)
{
	var onUrlChange = impl.aC;
	var onUrlRequest = impl.aD;
	var key = function() { key.a(onUrlChange(_Browser_getUrl())); };

	return _Browser_document({
		Q: function(sendToApp)
		{
			key.a = sendToApp;
			_Browser_window.addEventListener('popstate', key);
			_Browser_window.navigator.userAgent.indexOf('Trident') < 0 || _Browser_window.addEventListener('hashchange', key);

			return F2(function(domNode, event)
			{
				if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button < 1 && !domNode.target && !domNode.hasAttribute('download'))
				{
					event.preventDefault();
					var href = domNode.href;
					var curr = _Browser_getUrl();
					var next = $elm$url$Url$fromString(href).a;
					sendToApp(onUrlRequest(
						(next
							&& curr._ === next._
							&& curr.Y === next.Y
							&& curr.Z.a === next.Z.a
						)
							? $elm$browser$Browser$Internal(next)
							: $elm$browser$Browser$External(href)
					));
				}
			});
		},
		f: function(flags)
		{
			return A3(impl.f, flags, _Browser_getUrl(), key);
		},
		k: impl.k,
		h: impl.h,
		j: impl.j
	});
}

function _Browser_getUrl()
{
	return $elm$url$Url$fromString(_VirtualDom_doc.location.href).a || _Debug_crash(1);
}

var _Browser_go = F2(function(key, n)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		n && history.go(n);
		key();
	}));
});

var _Browser_pushUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.pushState({}, '', url);
		key();
	}));
});

var _Browser_replaceUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.replaceState({}, '', url);
		key();
	}));
});



// GLOBAL EVENTS


var _Browser_fakeNode = { addEventListener: function() {}, removeEventListener: function() {} };
var _Browser_doc = typeof document !== 'undefined' ? document : _Browser_fakeNode;
var _Browser_window = typeof window !== 'undefined' ? window : _Browser_fakeNode;

var _Browser_on = F3(function(node, eventName, sendToSelf)
{
	return _Scheduler_spawn(_Scheduler_binding(function(callback)
	{
		function handler(event)	{ _Scheduler_rawSpawn(sendToSelf(event)); }
		node.addEventListener(eventName, handler, _VirtualDom_passiveSupported && { passive: true });
		return function() { node.removeEventListener(eventName, handler); };
	}));
});

var _Browser_decodeEvent = F2(function(decoder, event)
{
	var result = _Json_runHelp(decoder, event);
	return $elm$core$Result$isOk(result) ? $elm$core$Maybe$Just(result.a) : $elm$core$Maybe$Nothing;
});



// PAGE VISIBILITY


function _Browser_visibilityInfo()
{
	return (typeof _VirtualDom_doc.hidden !== 'undefined')
		? { M: 'hidden', J: 'visibilitychange' }
		:
	(typeof _VirtualDom_doc.mozHidden !== 'undefined')
		? { M: 'mozHidden', J: 'mozvisibilitychange' }
		:
	(typeof _VirtualDom_doc.msHidden !== 'undefined')
		? { M: 'msHidden', J: 'msvisibilitychange' }
		:
	(typeof _VirtualDom_doc.webkitHidden !== 'undefined')
		? { M: 'webkitHidden', J: 'webkitvisibilitychange' }
		: { M: 'hidden', J: 'visibilitychange' };
}



// ANIMATION FRAMES


function _Browser_rAF()
{
	return _Scheduler_binding(function(callback)
	{
		var id = _Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(Date.now()));
		});

		return function() {
			_Browser_cancelAnimationFrame(id);
		};
	});
}


function _Browser_now()
{
	return _Scheduler_binding(function(callback)
	{
		callback(_Scheduler_succeed(Date.now()));
	});
}



// DOM STUFF


function _Browser_withNode(id, doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			var node = document.getElementById(id);
			callback(node
				? _Scheduler_succeed(doStuff(node))
				: _Scheduler_fail($elm$browser$Browser$Dom$NotFound(id))
			);
		});
	});
}


function _Browser_withWindow(doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(doStuff()));
		});
	});
}


// FOCUS and BLUR


var _Browser_call = F2(function(functionName, id)
{
	return _Browser_withNode(id, function(node) {
		node[functionName]();
		return _Utils_Tuple0;
	});
});



// WINDOW VIEWPORT


function _Browser_getViewport()
{
	return {
		P: _Browser_getScene(),
		W: {
			A: _Browser_window.pageXOffset,
			B: _Browser_window.pageYOffset,
			p: _Browser_doc.documentElement.clientWidth,
			o: _Browser_doc.documentElement.clientHeight
		}
	};
}

function _Browser_getScene()
{
	var body = _Browser_doc.body;
	var elem = _Browser_doc.documentElement;
	return {
		p: Math.max(body.scrollWidth, body.offsetWidth, elem.scrollWidth, elem.offsetWidth, elem.clientWidth),
		o: Math.max(body.scrollHeight, body.offsetHeight, elem.scrollHeight, elem.offsetHeight, elem.clientHeight)
	};
}

var _Browser_setViewport = F2(function(x, y)
{
	return _Browser_withWindow(function()
	{
		_Browser_window.scroll(x, y);
		return _Utils_Tuple0;
	});
});



// ELEMENT VIEWPORT


function _Browser_getViewportOf(id)
{
	return _Browser_withNode(id, function(node)
	{
		return {
			P: {
				p: node.scrollWidth,
				o: node.scrollHeight
			},
			W: {
				A: node.scrollLeft,
				B: node.scrollTop,
				p: node.clientWidth,
				o: node.clientHeight
			}
		};
	});
}


var _Browser_setViewportOf = F3(function(id, x, y)
{
	return _Browser_withNode(id, function(node)
	{
		node.scrollLeft = x;
		node.scrollTop = y;
		return _Utils_Tuple0;
	});
});



// ELEMENT


function _Browser_getElement(id)
{
	return _Browser_withNode(id, function(node)
	{
		var rect = node.getBoundingClientRect();
		var x = _Browser_window.pageXOffset;
		var y = _Browser_window.pageYOffset;
		return {
			P: _Browser_getScene(),
			W: {
				A: x,
				B: y,
				p: _Browser_doc.documentElement.clientWidth,
				o: _Browser_doc.documentElement.clientHeight
			},
			ax: {
				A: x + rect.left,
				B: y + rect.top,
				p: rect.width,
				o: rect.height
			}
		};
	});
}



// LOAD and RELOAD


function _Browser_reload(skipCache)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		_VirtualDom_doc.location.reload(skipCache);
	}));
}

function _Browser_load(url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		try
		{
			_Browser_window.location = url;
		}
		catch(err)
		{
			// Only Firefox can throw a NS_ERROR_MALFORMED_URI exception here.
			// Other browsers reload the page, so let's be consistent about that.
			_VirtualDom_doc.location.reload(false);
		}
	}));
}





// HELPERS


function _Debugger_unsafeCoerce(value)
{
	return value;
}



// PROGRAMS


var _Debugger_element = F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		A3($elm$browser$Debugger$Main$wrapInit, _Json_wrap(debugMetadata), _Debugger_popout(), impl.f),
		$elm$browser$Debugger$Main$wrapUpdate(impl.h),
		$elm$browser$Debugger$Main$wrapSubs(impl.j),
		function(sendToApp, initialModel)
		{
			var view = impl.k;
			var title = _VirtualDom_doc.title;
			var domNode = args && args['node'] ? args['node'] : _Debug_crash(0);
			var currNode = _VirtualDom_virtualize(domNode);
			var currBlocker = $elm$browser$Debugger$Main$toBlockerType(initialModel);
			var currPopout;

			var cornerNode = _VirtualDom_doc.createElement('div');
			domNode.parentNode.insertBefore(cornerNode, domNode.nextSibling);
			var cornerCurr = _VirtualDom_virtualize(cornerNode);

			initialModel.i.a = sendToApp;

			return _Browser_makeAnimator(initialModel, function(model)
			{
				var nextNode = A2(_VirtualDom_map, $elm$browser$Debugger$Main$UserMsg, view($elm$browser$Debugger$Main$getUserModel(model)));
				var patches = _VirtualDom_diff(currNode, nextNode);
				domNode = _VirtualDom_applyPatches(domNode, currNode, patches, sendToApp);
				currNode = nextNode;

				// update blocker

				var nextBlocker = $elm$browser$Debugger$Main$toBlockerType(model);
				_Debugger_updateBlocker(currBlocker, nextBlocker);
				currBlocker = nextBlocker;

				// view corner

				if (!model.i.b)
				{
					var cornerNext = $elm$browser$Debugger$Main$cornerView(model);
					var cornerPatches = _VirtualDom_diff(cornerCurr, cornerNext);
					cornerNode = _VirtualDom_applyPatches(cornerNode, cornerCurr, cornerPatches, sendToApp);
					cornerCurr = cornerNext;
					currPopout = undefined;
					return;
				}

				// view popout

				_VirtualDom_doc = model.i.b; // SWITCH TO POPOUT DOC
				currPopout || (currPopout = _VirtualDom_virtualize(model.i.b));
				var nextPopout = $elm$browser$Debugger$Main$popoutView(model);
				var popoutPatches = _VirtualDom_diff(currPopout, nextPopout);
				_VirtualDom_applyPatches(model.i.b.body, currPopout, popoutPatches, sendToApp);
				currPopout = nextPopout;
				_VirtualDom_doc = document; // SWITCH BACK TO NORMAL DOC
			});
		}
	);
});


var _Debugger_document = F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		A3($elm$browser$Debugger$Main$wrapInit, _Json_wrap(debugMetadata), _Debugger_popout(), impl.f),
		$elm$browser$Debugger$Main$wrapUpdate(impl.h),
		$elm$browser$Debugger$Main$wrapSubs(impl.j),
		function(sendToApp, initialModel)
		{
			var divertHrefToApp = impl.Q && impl.Q(function(x) { return sendToApp($elm$browser$Debugger$Main$UserMsg(x)); });
			var view = impl.k;
			var title = _VirtualDom_doc.title;
			var bodyNode = _VirtualDom_doc.body;
			var currNode = _VirtualDom_virtualize(bodyNode);
			var currBlocker = $elm$browser$Debugger$Main$toBlockerType(initialModel);
			var currPopout;

			initialModel.i.a = sendToApp;

			return _Browser_makeAnimator(initialModel, function(model)
			{
				_VirtualDom_divertHrefToApp = divertHrefToApp;
				var doc = view($elm$browser$Debugger$Main$getUserModel(model));
				var nextNode = _VirtualDom_node('body')(_List_Nil)(
					_Utils_ap(
						A2($elm$core$List$map, _VirtualDom_map($elm$browser$Debugger$Main$UserMsg), doc.v),
						_List_Cons($elm$browser$Debugger$Main$cornerView(model), _List_Nil)
					)
				);
				var patches = _VirtualDom_diff(currNode, nextNode);
				bodyNode = _VirtualDom_applyPatches(bodyNode, currNode, patches, sendToApp);
				currNode = nextNode;
				_VirtualDom_divertHrefToApp = 0;
				(title !== doc.a) && (_VirtualDom_doc.title = title = doc.a);

				// update blocker

				var nextBlocker = $elm$browser$Debugger$Main$toBlockerType(model);
				_Debugger_updateBlocker(currBlocker, nextBlocker);
				currBlocker = nextBlocker;

				// view popout

				if (!model.i.b) { currPopout = undefined; return; }

				_VirtualDom_doc = model.i.b; // SWITCH TO POPOUT DOC
				currPopout || (currPopout = _VirtualDom_virtualize(model.i.b));
				var nextPopout = $elm$browser$Debugger$Main$popoutView(model);
				var popoutPatches = _VirtualDom_diff(currPopout, nextPopout);
				_VirtualDom_applyPatches(model.i.b.body, currPopout, popoutPatches, sendToApp);
				currPopout = nextPopout;
				_VirtualDom_doc = document; // SWITCH BACK TO NORMAL DOC
			});
		}
	);
});


function _Debugger_popout()
{
	return {
		b: undefined,
		a: undefined
	};
}

function _Debugger_isOpen(popout)
{
	return !!popout.b;
}

function _Debugger_open(popout)
{
	return _Scheduler_binding(function(callback)
	{
		_Debugger_openWindow(popout);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
}

function _Debugger_openWindow(popout)
{
	var w = 900, h = 360, x = screen.width - w, y = screen.height - h;
	var debuggerWindow = window.open('', '', 'width=' + w + ',height=' + h + ',left=' + x + ',top=' + y);
	var doc = debuggerWindow.document;
	doc.title = 'Elm Debugger';

	// handle arrow keys
	doc.addEventListener('keydown', function(event) {
		event.metaKey && event.which === 82 && window.location.reload();
		event.which === 38 && (popout.a($elm$browser$Debugger$Main$Up), event.preventDefault());
		event.which === 40 && (popout.a($elm$browser$Debugger$Main$Down), event.preventDefault());
	});

	// handle window close
	window.addEventListener('unload', close);
	debuggerWindow.addEventListener('unload', function() {
		popout.b = undefined;
		popout.a($elm$browser$Debugger$Main$NoOp);
		window.removeEventListener('unload', close);
	});
	function close() {
		popout.b = undefined;
		popout.a($elm$browser$Debugger$Main$NoOp);
		debuggerWindow.close();
	}

	// register new window
	popout.b = doc;
}



// SCROLL


function _Debugger_scroll(popout)
{
	return _Scheduler_binding(function(callback)
	{
		if (popout.b)
		{
			var msgs = popout.b.getElementById('elm-debugger-sidebar');
			if (msgs)
			{
				msgs.scrollTop = msgs.scrollHeight;
			}
		}
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
}



// UPLOAD


function _Debugger_upload()
{
	return _Scheduler_binding(function(callback)
	{
		var element = document.createElement('input');
		element.setAttribute('type', 'file');
		element.setAttribute('accept', 'text/json');
		element.style.display = 'none';
		element.addEventListener('change', function(event)
		{
			var fileReader = new FileReader();
			fileReader.onload = function(e)
			{
				callback(_Scheduler_succeed(e.target.result));
			};
			fileReader.readAsText(event.target.files[0]);
			document.body.removeChild(element);
		});
		document.body.appendChild(element);
		element.click();
	});
}



// DOWNLOAD


var _Debugger_download = F2(function(historyLength, json)
{
	return _Scheduler_binding(function(callback)
	{
		var fileName = 'history-' + historyLength + '.txt';
		var jsonString = JSON.stringify(json);
		var mime = 'text/plain;charset=utf-8';
		var done = _Scheduler_succeed(_Utils_Tuple0);

		// for IE10+
		if (navigator.msSaveBlob)
		{
			navigator.msSaveBlob(new Blob([jsonString], {type: mime}), fileName);
			return callback(done);
		}

		// for HTML5
		var element = document.createElement('a');
		element.setAttribute('href', 'data:' + mime + ',' + encodeURIComponent(jsonString));
		element.setAttribute('download', fileName);
		element.style.display = 'none';
		document.body.appendChild(element);
		element.click();
		document.body.removeChild(element);
		callback(done);
	});
});



// POPOUT CONTENT


function _Debugger_messageToString(value)
{
	if (typeof value === 'boolean')
	{
		return value ? 'True' : 'False';
	}

	if (typeof value === 'number')
	{
		return value + '';
	}

	if (typeof value === 'string')
	{
		return '"' + _Debugger_addSlashes(value, false) + '"';
	}

	if (value instanceof String)
	{
		return "'" + _Debugger_addSlashes(value, true) + "'";
	}

	if (typeof value !== 'object' || value === null || !('$' in value))
	{
		return '…';
	}

	if (typeof value.$ === 'number')
	{
		return '…';
	}

	var code = value.$.charCodeAt(0);
	if (code === 0x23 /* # */ || /* a */ 0x61 <= code && code <= 0x7A /* z */)
	{
		return '…';
	}

	if (['Array_elm_builtin', 'Set_elm_builtin', 'RBNode_elm_builtin', 'RBEmpty_elm_builtin'].indexOf(value.$) >= 0)
	{
		return '…';
	}

	var keys = Object.keys(value);
	switch (keys.length)
	{
		case 1:
			return value.$;
		case 2:
			return value.$ + ' ' + _Debugger_messageToString(value.a);
		default:
			return value.$ + ' … ' + _Debugger_messageToString(value[keys[keys.length - 1]]);
	}
}


function _Debugger_init(value)
{
	if (typeof value === 'boolean')
	{
		return A3($elm$browser$Debugger$Expando$Constructor, $elm$core$Maybe$Just(value ? 'True' : 'False'), true, _List_Nil);
	}

	if (typeof value === 'number')
	{
		return $elm$browser$Debugger$Expando$Primitive(value + '');
	}

	if (typeof value === 'string')
	{
		return $elm$browser$Debugger$Expando$S('"' + _Debugger_addSlashes(value, false) + '"');
	}

	if (value instanceof String)
	{
		return $elm$browser$Debugger$Expando$S("'" + _Debugger_addSlashes(value, true) + "'");
	}

	if (typeof value === 'object' && '$' in value)
	{
		var tag = value.$;

		if (tag === '::' || tag === '[]')
		{
			return A3($elm$browser$Debugger$Expando$Sequence, $elm$browser$Debugger$Expando$ListSeq, true,
				A2($elm$core$List$map, _Debugger_init, value)
			);
		}

		if (tag === 'Set_elm_builtin')
		{
			return A3($elm$browser$Debugger$Expando$Sequence, $elm$browser$Debugger$Expando$SetSeq, true,
				A3($elm$core$Set$foldr, _Debugger_initCons, _List_Nil, value)
			);
		}

		if (tag === 'RBNode_elm_builtin' || tag == 'RBEmpty_elm_builtin')
		{
			return A2($elm$browser$Debugger$Expando$Dictionary, true,
				A3($elm$core$Dict$foldr, _Debugger_initKeyValueCons, _List_Nil, value)
			);
		}

		if (tag === 'Array_elm_builtin')
		{
			return A3($elm$browser$Debugger$Expando$Sequence, $elm$browser$Debugger$Expando$ArraySeq, true,
				A3($elm$core$Array$foldr, _Debugger_initCons, _List_Nil, value)
			);
		}

		if (typeof tag === 'number')
		{
			return $elm$browser$Debugger$Expando$Primitive('<internals>');
		}

		var char = tag.charCodeAt(0);
		if (char === 35 || 65 <= char && char <= 90)
		{
			var list = _List_Nil;
			for (var i in value)
			{
				if (i === '$') continue;
				list = _List_Cons(_Debugger_init(value[i]), list);
			}
			return A3($elm$browser$Debugger$Expando$Constructor, char === 35 ? $elm$core$Maybe$Nothing : $elm$core$Maybe$Just(tag), true, $elm$core$List$reverse(list));
		}

		return $elm$browser$Debugger$Expando$Primitive('<internals>');
	}

	if (typeof value === 'object')
	{
		var dict = $elm$core$Dict$empty;
		for (var i in value)
		{
			dict = A3($elm$core$Dict$insert, i, _Debugger_init(value[i]), dict);
		}
		return A2($elm$browser$Debugger$Expando$Record, true, dict);
	}

	return $elm$browser$Debugger$Expando$Primitive('<internals>');
}

var _Debugger_initCons = F2(function initConsHelp(value, list)
{
	return _List_Cons(_Debugger_init(value), list);
});

var _Debugger_initKeyValueCons = F3(function(key, value, list)
{
	return _List_Cons(
		_Utils_Tuple2(_Debugger_init(key), _Debugger_init(value)),
		list
	);
});

function _Debugger_addSlashes(str, isChar)
{
	var s = str
		.replace(/\\/g, '\\\\')
		.replace(/\n/g, '\\n')
		.replace(/\t/g, '\\t')
		.replace(/\r/g, '\\r')
		.replace(/\v/g, '\\v')
		.replace(/\0/g, '\\0');
	if (isChar)
	{
		return s.replace(/\'/g, '\\\'');
	}
	else
	{
		return s.replace(/\"/g, '\\"');
	}
}



// BLOCK EVENTS


function _Debugger_updateBlocker(oldBlocker, newBlocker)
{
	if (oldBlocker === newBlocker) return;

	var oldEvents = _Debugger_blockerToEvents(oldBlocker);
	var newEvents = _Debugger_blockerToEvents(newBlocker);

	// remove old blockers
	for (var i = 0; i < oldEvents.length; i++)
	{
		document.removeEventListener(oldEvents[i], _Debugger_blocker, true);
	}

	// add new blockers
	for (var i = 0; i < newEvents.length; i++)
	{
		document.addEventListener(newEvents[i], _Debugger_blocker, true);
	}
}


function _Debugger_blocker(event)
{
	if (event.type === 'keydown' && event.metaKey && event.which === 82)
	{
		return;
	}

	var isScroll = event.type === 'scroll' || event.type === 'wheel';
	for (var node = event.target; node; node = node.parentNode)
	{
		if (isScroll ? node.id === 'elm-debugger-details' : node.id === 'elm-debugger-overlay')
		{
			return;
		}
	}

	event.stopPropagation();
	event.preventDefault();
}

function _Debugger_blockerToEvents(blocker)
{
	return blocker === $elm$browser$Debugger$Overlay$BlockNone
		? []
		: blocker === $elm$browser$Debugger$Overlay$BlockMost
			? _Debugger_mostEvents
			: _Debugger_allEvents;
}

var _Debugger_mostEvents = [
	'click', 'dblclick', 'mousemove',
	'mouseup', 'mousedown', 'mouseenter', 'mouseleave',
	'touchstart', 'touchend', 'touchcancel', 'touchmove',
	'pointerdown', 'pointerup', 'pointerover', 'pointerout',
	'pointerenter', 'pointerleave', 'pointermove', 'pointercancel',
	'dragstart', 'drag', 'dragend', 'dragenter', 'dragover', 'dragleave', 'drop',
	'keyup', 'keydown', 'keypress',
	'input', 'change',
	'focus', 'blur'
];

var _Debugger_allEvents = _Debugger_mostEvents.concat('wheel', 'scroll');






// HELPERS


var _VirtualDom_divertHrefToApp;

var _VirtualDom_doc = typeof document !== 'undefined' ? document : {};


function _VirtualDom_appendChild(parent, child)
{
	parent.appendChild(child);
}

var _VirtualDom_init = F4(function(virtualNode, flagDecoder, debugMetadata, args)
{
	// NOTE: this function needs _Platform_export available to work

	/**/
	var node = args['node'];
	//*/
	/**_UNUSED/
	var node = args && args['node'] ? args['node'] : _Debug_crash(0);
	//*/

	node.parentNode.replaceChild(
		_VirtualDom_render(virtualNode, function() {}),
		node
	);

	return {};
});



// TEXT


function _VirtualDom_text(string)
{
	return {
		$: 0,
		a: string
	};
}



// NODE


var _VirtualDom_nodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 1,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_node = _VirtualDom_nodeNS(undefined);



// KEYED NODE


var _VirtualDom_keyedNodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 2,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_keyedNode = _VirtualDom_keyedNodeNS(undefined);



// CUSTOM


function _VirtualDom_custom(factList, model, render, diff)
{
	return {
		$: 3,
		d: _VirtualDom_organizeFacts(factList),
		g: model,
		h: render,
		i: diff
	};
}



// MAP


var _VirtualDom_map = F2(function(tagger, node)
{
	return {
		$: 4,
		j: tagger,
		k: node,
		b: 1 + (node.b || 0)
	};
});



// LAZY


function _VirtualDom_thunk(refs, thunk)
{
	return {
		$: 5,
		l: refs,
		m: thunk,
		k: undefined
	};
}

var _VirtualDom_lazy = F2(function(func, a)
{
	return _VirtualDom_thunk([func, a], function() {
		return func(a);
	});
});

var _VirtualDom_lazy2 = F3(function(func, a, b)
{
	return _VirtualDom_thunk([func, a, b], function() {
		return A2(func, a, b);
	});
});

var _VirtualDom_lazy3 = F4(function(func, a, b, c)
{
	return _VirtualDom_thunk([func, a, b, c], function() {
		return A3(func, a, b, c);
	});
});

var _VirtualDom_lazy4 = F5(function(func, a, b, c, d)
{
	return _VirtualDom_thunk([func, a, b, c, d], function() {
		return A4(func, a, b, c, d);
	});
});

var _VirtualDom_lazy5 = F6(function(func, a, b, c, d, e)
{
	return _VirtualDom_thunk([func, a, b, c, d, e], function() {
		return A5(func, a, b, c, d, e);
	});
});

var _VirtualDom_lazy6 = F7(function(func, a, b, c, d, e, f)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f], function() {
		return A6(func, a, b, c, d, e, f);
	});
});

var _VirtualDom_lazy7 = F8(function(func, a, b, c, d, e, f, g)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g], function() {
		return A7(func, a, b, c, d, e, f, g);
	});
});

var _VirtualDom_lazy8 = F9(function(func, a, b, c, d, e, f, g, h)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g, h], function() {
		return A8(func, a, b, c, d, e, f, g, h);
	});
});



// FACTS


var _VirtualDom_on = F2(function(key, handler)
{
	return {
		$: 'a0',
		n: key,
		o: handler
	};
});
var _VirtualDom_style = F2(function(key, value)
{
	return {
		$: 'a1',
		n: key,
		o: value
	};
});
var _VirtualDom_property = F2(function(key, value)
{
	return {
		$: 'a2',
		n: key,
		o: value
	};
});
var _VirtualDom_attribute = F2(function(key, value)
{
	return {
		$: 'a3',
		n: key,
		o: value
	};
});
var _VirtualDom_attributeNS = F3(function(namespace, key, value)
{
	return {
		$: 'a4',
		n: key,
		o: { f: namespace, o: value }
	};
});



// XSS ATTACK VECTOR CHECKS


function _VirtualDom_noScript(tag)
{
	return tag == 'script' ? 'p' : tag;
}

function _VirtualDom_noOnOrFormAction(key)
{
	return /^(on|formAction$)/i.test(key) ? 'data-' + key : key;
}

function _VirtualDom_noInnerHtmlOrFormAction(key)
{
	return key == 'innerHTML' || key == 'formAction' ? 'data-' + key : key;
}

function _VirtualDom_noJavaScriptUri(value)
{
	return /^javascript:/i.test(value.replace(/\s/g,'')) ? '' : value;
}

function _VirtualDom_noJavaScriptUri_UNUSED(value)
{
	return /^javascript:/i.test(value.replace(/\s/g,''))
		? 'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'
		: value;
}

function _VirtualDom_noJavaScriptOrHtmlUri(value)
{
	return /^\s*(javascript:|data:text\/html)/i.test(value) ? '' : value;
}

function _VirtualDom_noJavaScriptOrHtmlUri_UNUSED(value)
{
	return /^\s*(javascript:|data:text\/html)/i.test(value)
		? 'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'
		: value;
}



// MAP FACTS


var _VirtualDom_mapAttribute = F2(function(func, attr)
{
	return (attr.$ === 'a0')
		? A2(_VirtualDom_on, attr.n, _VirtualDom_mapHandler(func, attr.o))
		: attr;
});

function _VirtualDom_mapHandler(func, handler)
{
	var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

	// 0 = Normal
	// 1 = MayStopPropagation
	// 2 = MayPreventDefault
	// 3 = Custom

	return {
		$: handler.$,
		a:
			!tag
				? A2($elm$json$Json$Decode$map, func, handler.a)
				:
			A3($elm$json$Json$Decode$map2,
				tag < 3
					? _VirtualDom_mapEventTuple
					: _VirtualDom_mapEventRecord,
				$elm$json$Json$Decode$succeed(func),
				handler.a
			)
	};
}

var _VirtualDom_mapEventTuple = F2(function(func, tuple)
{
	return _Utils_Tuple2(func(tuple.a), tuple.b);
});

var _VirtualDom_mapEventRecord = F2(function(func, record)
{
	return {
		b: func(record.b),
		R: record.R,
		N: record.N
	}
});



// ORGANIZE FACTS


function _VirtualDom_organizeFacts(factList)
{
	for (var facts = {}; factList.b; factList = factList.b) // WHILE_CONS
	{
		var entry = factList.a;

		var tag = entry.$;
		var key = entry.n;
		var value = entry.o;

		if (tag === 'a2')
		{
			(key === 'className')
				? _VirtualDom_addClass(facts, key, _Json_unwrap(value))
				: facts[key] = _Json_unwrap(value);

			continue;
		}

		var subFacts = facts[tag] || (facts[tag] = {});
		(tag === 'a3' && key === 'class')
			? _VirtualDom_addClass(subFacts, key, value)
			: subFacts[key] = value;
	}

	return facts;
}

function _VirtualDom_addClass(object, key, newClass)
{
	var classes = object[key];
	object[key] = classes ? classes + ' ' + newClass : newClass;
}



// RENDER


function _VirtualDom_render(vNode, eventNode)
{
	var tag = vNode.$;

	if (tag === 5)
	{
		return _VirtualDom_render(vNode.k || (vNode.k = vNode.m()), eventNode);
	}

	if (tag === 0)
	{
		return _VirtualDom_doc.createTextNode(vNode.a);
	}

	if (tag === 4)
	{
		var subNode = vNode.k;
		var tagger = vNode.j;

		while (subNode.$ === 4)
		{
			typeof tagger !== 'object'
				? tagger = [tagger, subNode.j]
				: tagger.push(subNode.j);

			subNode = subNode.k;
		}

		var subEventRoot = { j: tagger, p: eventNode };
		var domNode = _VirtualDom_render(subNode, subEventRoot);
		domNode.elm_event_node_ref = subEventRoot;
		return domNode;
	}

	if (tag === 3)
	{
		var domNode = vNode.h(vNode.g);
		_VirtualDom_applyFacts(domNode, eventNode, vNode.d);
		return domNode;
	}

	// at this point `tag` must be 1 or 2

	var domNode = vNode.f
		? _VirtualDom_doc.createElementNS(vNode.f, vNode.c)
		: _VirtualDom_doc.createElement(vNode.c);

	if (_VirtualDom_divertHrefToApp && vNode.c == 'a')
	{
		domNode.addEventListener('click', _VirtualDom_divertHrefToApp(domNode));
	}

	_VirtualDom_applyFacts(domNode, eventNode, vNode.d);

	for (var kids = vNode.e, i = 0; i < kids.length; i++)
	{
		_VirtualDom_appendChild(domNode, _VirtualDom_render(tag === 1 ? kids[i] : kids[i].b, eventNode));
	}

	return domNode;
}



// APPLY FACTS


function _VirtualDom_applyFacts(domNode, eventNode, facts)
{
	for (var key in facts)
	{
		var value = facts[key];

		key === 'a1'
			? _VirtualDom_applyStyles(domNode, value)
			:
		key === 'a0'
			? _VirtualDom_applyEvents(domNode, eventNode, value)
			:
		key === 'a3'
			? _VirtualDom_applyAttrs(domNode, value)
			:
		key === 'a4'
			? _VirtualDom_applyAttrsNS(domNode, value)
			:
		((key !== 'value' && key !== 'checked') || domNode[key] !== value) && (domNode[key] = value);
	}
}



// APPLY STYLES


function _VirtualDom_applyStyles(domNode, styles)
{
	var domNodeStyle = domNode.style;

	for (var key in styles)
	{
		domNodeStyle[key] = styles[key];
	}
}



// APPLY ATTRS


function _VirtualDom_applyAttrs(domNode, attrs)
{
	for (var key in attrs)
	{
		var value = attrs[key];
		typeof value !== 'undefined'
			? domNode.setAttribute(key, value)
			: domNode.removeAttribute(key);
	}
}



// APPLY NAMESPACED ATTRS


function _VirtualDom_applyAttrsNS(domNode, nsAttrs)
{
	for (var key in nsAttrs)
	{
		var pair = nsAttrs[key];
		var namespace = pair.f;
		var value = pair.o;

		typeof value !== 'undefined'
			? domNode.setAttributeNS(namespace, key, value)
			: domNode.removeAttributeNS(namespace, key);
	}
}



// APPLY EVENTS


function _VirtualDom_applyEvents(domNode, eventNode, events)
{
	var allCallbacks = domNode.elmFs || (domNode.elmFs = {});

	for (var key in events)
	{
		var newHandler = events[key];
		var oldCallback = allCallbacks[key];

		if (!newHandler)
		{
			domNode.removeEventListener(key, oldCallback);
			allCallbacks[key] = undefined;
			continue;
		}

		if (oldCallback)
		{
			var oldHandler = oldCallback.q;
			if (oldHandler.$ === newHandler.$)
			{
				oldCallback.q = newHandler;
				continue;
			}
			domNode.removeEventListener(key, oldCallback);
		}

		oldCallback = _VirtualDom_makeCallback(eventNode, newHandler);
		domNode.addEventListener(key, oldCallback,
			_VirtualDom_passiveSupported
			&& { passive: $elm$virtual_dom$VirtualDom$toHandlerInt(newHandler) < 2 }
		);
		allCallbacks[key] = oldCallback;
	}
}



// PASSIVE EVENTS


var _VirtualDom_passiveSupported;

try
{
	window.addEventListener('t', null, Object.defineProperty({}, 'passive', {
		get: function() { _VirtualDom_passiveSupported = true; }
	}));
}
catch(e) {}



// EVENT HANDLERS


function _VirtualDom_makeCallback(eventNode, initialHandler)
{
	function callback(event)
	{
		var handler = callback.q;
		var result = _Json_runHelp(handler.a, event);

		if (!$elm$core$Result$isOk(result))
		{
			return;
		}

		var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

		// 0 = Normal
		// 1 = MayStopPropagation
		// 2 = MayPreventDefault
		// 3 = Custom

		var value = result.a;
		var message = !tag ? value : tag < 3 ? value.a : value.b;
		var stopPropagation = tag == 1 ? value.b : tag == 3 && value.R;
		var currentEventNode = (
			stopPropagation && event.stopPropagation(),
			(tag == 2 ? value.b : tag == 3 && value.N) && event.preventDefault(),
			eventNode
		);
		var tagger;
		var i;
		while (tagger = currentEventNode.j)
		{
			if (typeof tagger == 'function')
			{
				message = tagger(message);
			}
			else
			{
				for (var i = tagger.length; i--; )
				{
					message = tagger[i](message);
				}
			}
			currentEventNode = currentEventNode.p;
		}
		currentEventNode(message, stopPropagation); // stopPropagation implies isSync
	}

	callback.q = initialHandler;

	return callback;
}

function _VirtualDom_equalEvents(x, y)
{
	return x.$ == y.$ && _Json_equality(x.a, y.a);
}



// DIFF


// TODO: Should we do patches like in iOS?
//
// type Patch
//   = At Int Patch
//   | Batch (List Patch)
//   | Change ...
//
// How could it not be better?
//
function _VirtualDom_diff(x, y)
{
	var patches = [];
	_VirtualDom_diffHelp(x, y, patches, 0);
	return patches;
}


function _VirtualDom_pushPatch(patches, type, index, data)
{
	var patch = {
		$: type,
		r: index,
		s: data,
		t: undefined,
		u: undefined
	};
	patches.push(patch);
	return patch;
}


function _VirtualDom_diffHelp(x, y, patches, index)
{
	if (x === y)
	{
		return;
	}

	var xType = x.$;
	var yType = y.$;

	// Bail if you run into different types of nodes. Implies that the
	// structure has changed significantly and it's not worth a diff.
	if (xType !== yType)
	{
		if (xType === 1 && yType === 2)
		{
			y = _VirtualDom_dekey(y);
			yType = 1;
		}
		else
		{
			_VirtualDom_pushPatch(patches, 0, index, y);
			return;
		}
	}

	// Now we know that both nodes are the same $.
	switch (yType)
	{
		case 5:
			var xRefs = x.l;
			var yRefs = y.l;
			var i = xRefs.length;
			var same = i === yRefs.length;
			while (same && i--)
			{
				same = xRefs[i] === yRefs[i];
			}
			if (same)
			{
				y.k = x.k;
				return;
			}
			y.k = y.m();
			var subPatches = [];
			_VirtualDom_diffHelp(x.k, y.k, subPatches, 0);
			subPatches.length > 0 && _VirtualDom_pushPatch(patches, 1, index, subPatches);
			return;

		case 4:
			// gather nested taggers
			var xTaggers = x.j;
			var yTaggers = y.j;
			var nesting = false;

			var xSubNode = x.k;
			while (xSubNode.$ === 4)
			{
				nesting = true;

				typeof xTaggers !== 'object'
					? xTaggers = [xTaggers, xSubNode.j]
					: xTaggers.push(xSubNode.j);

				xSubNode = xSubNode.k;
			}

			var ySubNode = y.k;
			while (ySubNode.$ === 4)
			{
				nesting = true;

				typeof yTaggers !== 'object'
					? yTaggers = [yTaggers, ySubNode.j]
					: yTaggers.push(ySubNode.j);

				ySubNode = ySubNode.k;
			}

			// Just bail if different numbers of taggers. This implies the
			// structure of the virtual DOM has changed.
			if (nesting && xTaggers.length !== yTaggers.length)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			// check if taggers are "the same"
			if (nesting ? !_VirtualDom_pairwiseRefEqual(xTaggers, yTaggers) : xTaggers !== yTaggers)
			{
				_VirtualDom_pushPatch(patches, 2, index, yTaggers);
			}

			// diff everything below the taggers
			_VirtualDom_diffHelp(xSubNode, ySubNode, patches, index + 1);
			return;

		case 0:
			if (x.a !== y.a)
			{
				_VirtualDom_pushPatch(patches, 3, index, y.a);
			}
			return;

		case 1:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKids);
			return;

		case 2:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKeyedKids);
			return;

		case 3:
			if (x.h !== y.h)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
			factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

			var patch = y.i(x.g, y.g);
			patch && _VirtualDom_pushPatch(patches, 5, index, patch);

			return;
	}
}

// assumes the incoming arrays are the same length
function _VirtualDom_pairwiseRefEqual(as, bs)
{
	for (var i = 0; i < as.length; i++)
	{
		if (as[i] !== bs[i])
		{
			return false;
		}
	}

	return true;
}

function _VirtualDom_diffNodes(x, y, patches, index, diffKids)
{
	// Bail if obvious indicators have changed. Implies more serious
	// structural changes such that it's not worth it to diff.
	if (x.c !== y.c || x.f !== y.f)
	{
		_VirtualDom_pushPatch(patches, 0, index, y);
		return;
	}

	var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
	factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

	diffKids(x, y, patches, index);
}



// DIFF FACTS


// TODO Instead of creating a new diff object, it's possible to just test if
// there *is* a diff. During the actual patch, do the diff again and make the
// modifications directly. This way, there's no new allocations. Worth it?
function _VirtualDom_diffFacts(x, y, category)
{
	var diff;

	// look for changes and removals
	for (var xKey in x)
	{
		if (xKey === 'a1' || xKey === 'a0' || xKey === 'a3' || xKey === 'a4')
		{
			var subDiff = _VirtualDom_diffFacts(x[xKey], y[xKey] || {}, xKey);
			if (subDiff)
			{
				diff = diff || {};
				diff[xKey] = subDiff;
			}
			continue;
		}

		// remove if not in the new facts
		if (!(xKey in y))
		{
			diff = diff || {};
			diff[xKey] =
				!category
					? (typeof x[xKey] === 'string' ? '' : null)
					:
				(category === 'a1')
					? ''
					:
				(category === 'a0' || category === 'a3')
					? undefined
					:
				{ f: x[xKey].f, o: undefined };

			continue;
		}

		var xValue = x[xKey];
		var yValue = y[xKey];

		// reference equal, so don't worry about it
		if (xValue === yValue && xKey !== 'value' && xKey !== 'checked'
			|| category === 'a0' && _VirtualDom_equalEvents(xValue, yValue))
		{
			continue;
		}

		diff = diff || {};
		diff[xKey] = yValue;
	}

	// add new stuff
	for (var yKey in y)
	{
		if (!(yKey in x))
		{
			diff = diff || {};
			diff[yKey] = y[yKey];
		}
	}

	return diff;
}



// DIFF KIDS


function _VirtualDom_diffKids(xParent, yParent, patches, index)
{
	var xKids = xParent.e;
	var yKids = yParent.e;

	var xLen = xKids.length;
	var yLen = yKids.length;

	// FIGURE OUT IF THERE ARE INSERTS OR REMOVALS

	if (xLen > yLen)
	{
		_VirtualDom_pushPatch(patches, 6, index, {
			v: yLen,
			i: xLen - yLen
		});
	}
	else if (xLen < yLen)
	{
		_VirtualDom_pushPatch(patches, 7, index, {
			v: xLen,
			e: yKids
		});
	}

	// PAIRWISE DIFF EVERYTHING ELSE

	for (var minLen = xLen < yLen ? xLen : yLen, i = 0; i < minLen; i++)
	{
		var xKid = xKids[i];
		_VirtualDom_diffHelp(xKid, yKids[i], patches, ++index);
		index += xKid.b || 0;
	}
}



// KEYED DIFF


function _VirtualDom_diffKeyedKids(xParent, yParent, patches, rootIndex)
{
	var localPatches = [];

	var changes = {}; // Dict String Entry
	var inserts = []; // Array { index : Int, entry : Entry }
	// type Entry = { tag : String, vnode : VNode, index : Int, data : _ }

	var xKids = xParent.e;
	var yKids = yParent.e;
	var xLen = xKids.length;
	var yLen = yKids.length;
	var xIndex = 0;
	var yIndex = 0;

	var index = rootIndex;

	while (xIndex < xLen && yIndex < yLen)
	{
		var x = xKids[xIndex];
		var y = yKids[yIndex];

		var xKey = x.a;
		var yKey = y.a;
		var xNode = x.b;
		var yNode = y.b;

		var newMatch = undefined;
		var oldMatch = undefined;

		// check if keys match

		if (xKey === yKey)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNode, localPatches, index);
			index += xNode.b || 0;

			xIndex++;
			yIndex++;
			continue;
		}

		// look ahead 1 to detect insertions and removals.

		var xNext = xKids[xIndex + 1];
		var yNext = yKids[yIndex + 1];

		if (xNext)
		{
			var xNextKey = xNext.a;
			var xNextNode = xNext.b;
			oldMatch = yKey === xNextKey;
		}

		if (yNext)
		{
			var yNextKey = yNext.a;
			var yNextNode = yNext.b;
			newMatch = xKey === yNextKey;
		}


		// swap x and y
		if (newMatch && oldMatch)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			_VirtualDom_insertNode(changes, localPatches, xKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNextNode, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		// insert y
		if (newMatch)
		{
			index++;
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			index += xNode.b || 0;

			xIndex += 1;
			yIndex += 2;
			continue;
		}

		// remove x
		if (oldMatch)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 1;
			continue;
		}

		// remove x, insert y
		if (xNext && xNextKey === yNextKey)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNextNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		break;
	}

	// eat up any remaining nodes with removeNode and insertNode

	while (xIndex < xLen)
	{
		index++;
		var x = xKids[xIndex];
		var xNode = x.b;
		_VirtualDom_removeNode(changes, localPatches, x.a, xNode, index);
		index += xNode.b || 0;
		xIndex++;
	}

	while (yIndex < yLen)
	{
		var endInserts = endInserts || [];
		var y = yKids[yIndex];
		_VirtualDom_insertNode(changes, localPatches, y.a, y.b, undefined, endInserts);
		yIndex++;
	}

	if (localPatches.length > 0 || inserts.length > 0 || endInserts)
	{
		_VirtualDom_pushPatch(patches, 8, rootIndex, {
			w: localPatches,
			x: inserts,
			y: endInserts
		});
	}
}



// CHANGES FROM KEYED DIFF


var _VirtualDom_POSTFIX = '_elmW6BL';


function _VirtualDom_insertNode(changes, localPatches, key, vnode, yIndex, inserts)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		entry = {
			c: 0,
			z: vnode,
			r: yIndex,
			s: undefined
		};

		inserts.push({ r: yIndex, A: entry });
		changes[key] = entry;

		return;
	}

	// this key was removed earlier, a match!
	if (entry.c === 1)
	{
		inserts.push({ r: yIndex, A: entry });

		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(entry.z, vnode, subPatches, entry.r);
		entry.r = yIndex;
		entry.s.s = {
			w: subPatches,
			A: entry
		};

		return;
	}

	// this key has already been inserted or moved, a duplicate!
	_VirtualDom_insertNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, yIndex, inserts);
}


function _VirtualDom_removeNode(changes, localPatches, key, vnode, index)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		var patch = _VirtualDom_pushPatch(localPatches, 9, index, undefined);

		changes[key] = {
			c: 1,
			z: vnode,
			r: index,
			s: patch
		};

		return;
	}

	// this key was inserted earlier, a match!
	if (entry.c === 0)
	{
		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(vnode, entry.z, subPatches, index);

		_VirtualDom_pushPatch(localPatches, 9, index, {
			w: subPatches,
			A: entry
		});

		return;
	}

	// this key has already been removed or moved, a duplicate!
	_VirtualDom_removeNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, index);
}



// ADD DOM NODES
//
// Each DOM node has an "index" assigned in order of traversal. It is important
// to minimize our crawl over the actual DOM, so these indexes (along with the
// descendantsCount of virtual nodes) let us skip touching entire subtrees of
// the DOM if we know there are no patches there.


function _VirtualDom_addDomNodes(domNode, vNode, patches, eventNode)
{
	_VirtualDom_addDomNodesHelp(domNode, vNode, patches, 0, 0, vNode.b, eventNode);
}


// assumes `patches` is non-empty and indexes increase monotonically.
function _VirtualDom_addDomNodesHelp(domNode, vNode, patches, i, low, high, eventNode)
{
	var patch = patches[i];
	var index = patch.r;

	while (index === low)
	{
		var patchType = patch.$;

		if (patchType === 1)
		{
			_VirtualDom_addDomNodes(domNode, vNode.k, patch.s, eventNode);
		}
		else if (patchType === 8)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var subPatches = patch.s.w;
			if (subPatches.length > 0)
			{
				_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
			}
		}
		else if (patchType === 9)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var data = patch.s;
			if (data)
			{
				data.A.s = domNode;
				var subPatches = data.w;
				if (subPatches.length > 0)
				{
					_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
				}
			}
		}
		else
		{
			patch.t = domNode;
			patch.u = eventNode;
		}

		i++;

		if (!(patch = patches[i]) || (index = patch.r) > high)
		{
			return i;
		}
	}

	var tag = vNode.$;

	if (tag === 4)
	{
		var subNode = vNode.k;

		while (subNode.$ === 4)
		{
			subNode = subNode.k;
		}

		return _VirtualDom_addDomNodesHelp(domNode, subNode, patches, i, low + 1, high, domNode.elm_event_node_ref);
	}

	// tag must be 1 or 2 at this point

	var vKids = vNode.e;
	var childNodes = domNode.childNodes;
	for (var j = 0; j < vKids.length; j++)
	{
		low++;
		var vKid = tag === 1 ? vKids[j] : vKids[j].b;
		var nextLow = low + (vKid.b || 0);
		if (low <= index && index <= nextLow)
		{
			i = _VirtualDom_addDomNodesHelp(childNodes[j], vKid, patches, i, low, nextLow, eventNode);
			if (!(patch = patches[i]) || (index = patch.r) > high)
			{
				return i;
			}
		}
		low = nextLow;
	}
	return i;
}



// APPLY PATCHES


function _VirtualDom_applyPatches(rootDomNode, oldVirtualNode, patches, eventNode)
{
	if (patches.length === 0)
	{
		return rootDomNode;
	}

	_VirtualDom_addDomNodes(rootDomNode, oldVirtualNode, patches, eventNode);
	return _VirtualDom_applyPatchesHelp(rootDomNode, patches);
}

function _VirtualDom_applyPatchesHelp(rootDomNode, patches)
{
	for (var i = 0; i < patches.length; i++)
	{
		var patch = patches[i];
		var localDomNode = patch.t
		var newNode = _VirtualDom_applyPatch(localDomNode, patch);
		if (localDomNode === rootDomNode)
		{
			rootDomNode = newNode;
		}
	}
	return rootDomNode;
}

function _VirtualDom_applyPatch(domNode, patch)
{
	switch (patch.$)
	{
		case 0:
			return _VirtualDom_applyPatchRedraw(domNode, patch.s, patch.u);

		case 4:
			_VirtualDom_applyFacts(domNode, patch.u, patch.s);
			return domNode;

		case 3:
			domNode.replaceData(0, domNode.length, patch.s);
			return domNode;

		case 1:
			return _VirtualDom_applyPatchesHelp(domNode, patch.s);

		case 2:
			if (domNode.elm_event_node_ref)
			{
				domNode.elm_event_node_ref.j = patch.s;
			}
			else
			{
				domNode.elm_event_node_ref = { j: patch.s, p: patch.u };
			}
			return domNode;

		case 6:
			var data = patch.s;
			for (var i = 0; i < data.i; i++)
			{
				domNode.removeChild(domNode.childNodes[data.v]);
			}
			return domNode;

		case 7:
			var data = patch.s;
			var kids = data.e;
			var i = data.v;
			var theEnd = domNode.childNodes[i];
			for (; i < kids.length; i++)
			{
				domNode.insertBefore(_VirtualDom_render(kids[i], patch.u), theEnd);
			}
			return domNode;

		case 9:
			var data = patch.s;
			if (!data)
			{
				domNode.parentNode.removeChild(domNode);
				return domNode;
			}
			var entry = data.A;
			if (typeof entry.r !== 'undefined')
			{
				domNode.parentNode.removeChild(domNode);
			}
			entry.s = _VirtualDom_applyPatchesHelp(domNode, data.w);
			return domNode;

		case 8:
			return _VirtualDom_applyPatchReorder(domNode, patch);

		case 5:
			return patch.s(domNode);

		default:
			_Debug_crash(10); // 'Ran into an unknown patch!'
	}
}


function _VirtualDom_applyPatchRedraw(domNode, vNode, eventNode)
{
	var parentNode = domNode.parentNode;
	var newNode = _VirtualDom_render(vNode, eventNode);

	if (!newNode.elm_event_node_ref)
	{
		newNode.elm_event_node_ref = domNode.elm_event_node_ref;
	}

	if (parentNode && newNode !== domNode)
	{
		parentNode.replaceChild(newNode, domNode);
	}
	return newNode;
}


function _VirtualDom_applyPatchReorder(domNode, patch)
{
	var data = patch.s;

	// remove end inserts
	var frag = _VirtualDom_applyPatchReorderEndInsertsHelp(data.y, patch);

	// removals
	domNode = _VirtualDom_applyPatchesHelp(domNode, data.w);

	// inserts
	var inserts = data.x;
	for (var i = 0; i < inserts.length; i++)
	{
		var insert = inserts[i];
		var entry = insert.A;
		var node = entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u);
		domNode.insertBefore(node, domNode.childNodes[insert.r]);
	}

	// add end inserts
	if (frag)
	{
		_VirtualDom_appendChild(domNode, frag);
	}

	return domNode;
}


function _VirtualDom_applyPatchReorderEndInsertsHelp(endInserts, patch)
{
	if (!endInserts)
	{
		return;
	}

	var frag = _VirtualDom_doc.createDocumentFragment();
	for (var i = 0; i < endInserts.length; i++)
	{
		var insert = endInserts[i];
		var entry = insert.A;
		_VirtualDom_appendChild(frag, entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u)
		);
	}
	return frag;
}


function _VirtualDom_virtualize(node)
{
	// TEXT NODES

	if (node.nodeType === 3)
	{
		return _VirtualDom_text(node.textContent);
	}


	// WEIRD NODES

	if (node.nodeType !== 1)
	{
		return _VirtualDom_text('');
	}


	// ELEMENT NODES

	var attrList = _List_Nil;
	var attrs = node.attributes;
	for (var i = attrs.length; i--; )
	{
		var attr = attrs[i];
		var name = attr.name;
		var value = attr.value;
		attrList = _List_Cons( A2(_VirtualDom_attribute, name, value), attrList );
	}

	var tag = node.tagName.toLowerCase();
	var kidList = _List_Nil;
	var kids = node.childNodes;

	for (var i = kids.length; i--; )
	{
		kidList = _List_Cons(_VirtualDom_virtualize(kids[i]), kidList);
	}
	return A3(_VirtualDom_node, tag, attrList, kidList);
}

function _VirtualDom_dekey(keyedNode)
{
	var keyedKids = keyedNode.e;
	var len = keyedKids.length;
	var kids = new Array(len);
	for (var i = 0; i < len; i++)
	{
		kids[i] = keyedKids[i].b;
	}

	return {
		$: 1,
		c: keyedNode.c,
		d: keyedNode.d,
		e: kids,
		f: keyedNode.f,
		b: keyedNode.b
	};
}



function _Url_percentEncode(string)
{
	return encodeURIComponent(string);
}

function _Url_percentDecode(string)
{
	try
	{
		return $elm$core$Maybe$Just(decodeURIComponent(string));
	}
	catch (e)
	{
		return $elm$core$Maybe$Nothing;
	}
}



function _Process_sleep(time)
{
	return _Scheduler_binding(function(callback) {
		var id = setTimeout(function() {
			callback(_Scheduler_succeed(_Utils_Tuple0));
		}, time);

		return function() { clearTimeout(id); };
	});
}





// VIRTUAL-DOM WIDGETS


var _Markdown_toHtml = F3(function(options, factList, rawMarkdown)
{
	return _VirtualDom_custom(
		factList,
		{
			a: options,
			b: rawMarkdown
		},
		_Markdown_render,
		_Markdown_diff
	);
});



// WIDGET IMPLEMENTATION


function _Markdown_render(model)
{
	return A2(_Markdown_replace, model, _VirtualDom_doc.createElement('div'));
}


function _Markdown_diff(x, y)
{
	return x.b === y.b && x.a === y.a
		? false
		: _Markdown_replace(y);
}


var _Markdown_replace = F2(function(model, div)
{
	div.innerHTML = _Markdown_marked(model.b, _Markdown_formatOptions(model.a));
	return div;
});



// ACTUAL MARKDOWN PARSER


var _Markdown_marked = function() {
	// catch the `marked` object regardless of the outer environment.
	// (ex. a CommonJS module compatible environment.)
	// note that this depends on marked's implementation of environment detection.
	var module = {};
	var exports = module.exports = {};

	/**
	 * marked - a markdown parser
	 * Copyright (c) 2011-2014, Christopher Jeffrey. (MIT Licensed)
	 * https://github.com/chjj/marked
	 * commit cd2f6f5b7091154c5526e79b5f3bfb4d15995a51
	 */
	(function(){var block={newline:/^\n+/,code:/^( {4}[^\n]+\n*)+/,fences:noop,hr:/^( *[-*_]){3,} *(?:\n+|$)/,heading:/^ *(#{1,6}) *([^\n]+?) *#* *(?:\n+|$)/,nptable:noop,lheading:/^([^\n]+)\n *(=|-){2,} *(?:\n+|$)/,blockquote:/^( *>[^\n]+(\n(?!def)[^\n]+)*\n*)+/,list:/^( *)(bull) [\s\S]+?(?:hr|def|\n{2,}(?! )(?!\1bull )\n*|\s*$)/,html:/^ *(?:comment *(?:\n|\s*$)|closed *(?:\n{2,}|\s*$)|closing *(?:\n{2,}|\s*$))/,def:/^ *\[([^\]]+)\]: *<?([^\s>]+)>?(?: +["(]([^\n]+)[")])? *(?:\n+|$)/,table:noop,paragraph:/^((?:[^\n]+\n?(?!hr|heading|lheading|blockquote|tag|def))+)\n*/,text:/^[^\n]+/};block.bullet=/(?:[*+-]|\d+\.)/;block.item=/^( *)(bull) [^\n]*(?:\n(?!\1bull )[^\n]*)*/;block.item=replace(block.item,"gm")(/bull/g,block.bullet)();block.list=replace(block.list)(/bull/g,block.bullet)("hr","\\n+(?=\\1?(?:[-*_] *){3,}(?:\\n+|$))")("def","\\n+(?="+block.def.source+")")();block.blockquote=replace(block.blockquote)("def",block.def)();block._tag="(?!(?:"+"a|em|strong|small|s|cite|q|dfn|abbr|data|time|code"+"|var|samp|kbd|sub|sup|i|b|u|mark|ruby|rt|rp|bdi|bdo"+"|span|br|wbr|ins|del|img)\\b)\\w+(?!:/|[^\\w\\s@]*@)\\b";block.html=replace(block.html)("comment",/<!--[\s\S]*?-->/)("closed",/<(tag)[\s\S]+?<\/\1>/)("closing",/<tag(?:"[^"]*"|'[^']*'|[^'">])*?>/)(/tag/g,block._tag)();block.paragraph=replace(block.paragraph)("hr",block.hr)("heading",block.heading)("lheading",block.lheading)("blockquote",block.blockquote)("tag","<"+block._tag)("def",block.def)();block.normal=merge({},block);block.gfm=merge({},block.normal,{fences:/^ *(`{3,}|~{3,})[ \.]*(\S+)? *\n([\s\S]*?)\s*\1 *(?:\n+|$)/,paragraph:/^/,heading:/^ *(#{1,6}) +([^\n]+?) *#* *(?:\n+|$)/});block.gfm.paragraph=replace(block.paragraph)("(?!","(?!"+block.gfm.fences.source.replace("\\1","\\2")+"|"+block.list.source.replace("\\1","\\3")+"|")();block.tables=merge({},block.gfm,{nptable:/^ *(\S.*\|.*)\n *([-:]+ *\|[-| :]*)\n((?:.*\|.*(?:\n|$))*)\n*/,table:/^ *\|(.+)\n *\|( *[-:]+[-| :]*)\n((?: *\|.*(?:\n|$))*)\n*/});function Lexer(options){this.tokens=[];this.tokens.links={};this.options=options||marked.defaults;this.rules=block.normal;if(this.options.gfm){if(this.options.tables){this.rules=block.tables}else{this.rules=block.gfm}}}Lexer.rules=block;Lexer.lex=function(src,options){var lexer=new Lexer(options);return lexer.lex(src)};Lexer.prototype.lex=function(src){src=src.replace(/\r\n|\r/g,"\n").replace(/\t/g,"    ").replace(/\u00a0/g," ").replace(/\u2424/g,"\n");return this.token(src,true)};Lexer.prototype.token=function(src,top,bq){var src=src.replace(/^ +$/gm,""),next,loose,cap,bull,b,item,space,i,l;while(src){if(cap=this.rules.newline.exec(src)){src=src.substring(cap[0].length);if(cap[0].length>1){this.tokens.push({type:"space"})}}if(cap=this.rules.code.exec(src)){src=src.substring(cap[0].length);cap=cap[0].replace(/^ {4}/gm,"");this.tokens.push({type:"code",text:!this.options.pedantic?cap.replace(/\n+$/,""):cap});continue}if(cap=this.rules.fences.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"code",lang:cap[2],text:cap[3]||""});continue}if(cap=this.rules.heading.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"heading",depth:cap[1].length,text:cap[2]});continue}if(top&&(cap=this.rules.nptable.exec(src))){src=src.substring(cap[0].length);item={type:"table",header:cap[1].replace(/^ *| *\| *$/g,"").split(/ *\| */),align:cap[2].replace(/^ *|\| *$/g,"").split(/ *\| */),cells:cap[3].replace(/\n$/,"").split("\n")};for(i=0;i<item.align.length;i++){if(/^ *-+: *$/.test(item.align[i])){item.align[i]="right"}else if(/^ *:-+: *$/.test(item.align[i])){item.align[i]="center"}else if(/^ *:-+ *$/.test(item.align[i])){item.align[i]="left"}else{item.align[i]=null}}for(i=0;i<item.cells.length;i++){item.cells[i]=item.cells[i].split(/ *\| */)}this.tokens.push(item);continue}if(cap=this.rules.lheading.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"heading",depth:cap[2]==="="?1:2,text:cap[1]});continue}if(cap=this.rules.hr.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"hr"});continue}if(cap=this.rules.blockquote.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"blockquote_start"});cap=cap[0].replace(/^ *> ?/gm,"");this.token(cap,top,true);this.tokens.push({type:"blockquote_end"});continue}if(cap=this.rules.list.exec(src)){src=src.substring(cap[0].length);bull=cap[2];this.tokens.push({type:"list_start",ordered:bull.length>1});cap=cap[0].match(this.rules.item);next=false;l=cap.length;i=0;for(;i<l;i++){item=cap[i];space=item.length;item=item.replace(/^ *([*+-]|\d+\.) +/,"");if(~item.indexOf("\n ")){space-=item.length;item=!this.options.pedantic?item.replace(new RegExp("^ {1,"+space+"}","gm"),""):item.replace(/^ {1,4}/gm,"")}if(this.options.smartLists&&i!==l-1){b=block.bullet.exec(cap[i+1])[0];if(bull!==b&&!(bull.length>1&&b.length>1)){src=cap.slice(i+1).join("\n")+src;i=l-1}}loose=next||/\n\n(?!\s*$)/.test(item);if(i!==l-1){next=item.charAt(item.length-1)==="\n";if(!loose)loose=next}this.tokens.push({type:loose?"loose_item_start":"list_item_start"});this.token(item,false,bq);this.tokens.push({type:"list_item_end"})}this.tokens.push({type:"list_end"});continue}if(cap=this.rules.html.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:this.options.sanitize?"paragraph":"html",pre:!this.options.sanitizer&&(cap[1]==="pre"||cap[1]==="script"||cap[1]==="style"),text:cap[0]});continue}if(!bq&&top&&(cap=this.rules.def.exec(src))){src=src.substring(cap[0].length);this.tokens.links[cap[1].toLowerCase()]={href:cap[2],title:cap[3]};continue}if(top&&(cap=this.rules.table.exec(src))){src=src.substring(cap[0].length);item={type:"table",header:cap[1].replace(/^ *| *\| *$/g,"").split(/ *\| */),align:cap[2].replace(/^ *|\| *$/g,"").split(/ *\| */),cells:cap[3].replace(/(?: *\| *)?\n$/,"").split("\n")};for(i=0;i<item.align.length;i++){if(/^ *-+: *$/.test(item.align[i])){item.align[i]="right"}else if(/^ *:-+: *$/.test(item.align[i])){item.align[i]="center"}else if(/^ *:-+ *$/.test(item.align[i])){item.align[i]="left"}else{item.align[i]=null}}for(i=0;i<item.cells.length;i++){item.cells[i]=item.cells[i].replace(/^ *\| *| *\| *$/g,"").split(/ *\| */)}this.tokens.push(item);continue}if(top&&(cap=this.rules.paragraph.exec(src))){src=src.substring(cap[0].length);this.tokens.push({type:"paragraph",text:cap[1].charAt(cap[1].length-1)==="\n"?cap[1].slice(0,-1):cap[1]});continue}if(cap=this.rules.text.exec(src)){src=src.substring(cap[0].length);this.tokens.push({type:"text",text:cap[0]});continue}if(src){throw new Error("Infinite loop on byte: "+src.charCodeAt(0))}}return this.tokens};var inline={escape:/^\\([\\`*{}\[\]()#+\-.!_>])/,autolink:/^<([^ >]+(@|:\/)[^ >]+)>/,url:noop,tag:/^<!--[\s\S]*?-->|^<\/?\w+(?:"[^"]*"|'[^']*'|[^'">])*?>/,link:/^!?\[(inside)\]\(href\)/,reflink:/^!?\[(inside)\]\s*\[([^\]]*)\]/,nolink:/^!?\[((?:\[[^\]]*\]|[^\[\]])*)\]/,strong:/^_\_([\s\S]+?)_\_(?!_)|^\*\*([\s\S]+?)\*\*(?!\*)/,em:/^\b_((?:[^_]|_\_)+?)_\b|^\*((?:\*\*|[\s\S])+?)\*(?!\*)/,code:/^(`+)\s*([\s\S]*?[^`])\s*\1(?!`)/,br:/^ {2,}\n(?!\s*$)/,del:noop,text:/^[\s\S]+?(?=[\\<!\[_*`]| {2,}\n|$)/};inline._inside=/(?:\[[^\]]*\]|[^\[\]]|\](?=[^\[]*\]))*/;inline._href=/\s*<?([\s\S]*?)>?(?:\s+['"]([\s\S]*?)['"])?\s*/;inline.link=replace(inline.link)("inside",inline._inside)("href",inline._href)();inline.reflink=replace(inline.reflink)("inside",inline._inside)();inline.normal=merge({},inline);inline.pedantic=merge({},inline.normal,{strong:/^_\_(?=\S)([\s\S]*?\S)_\_(?!_)|^\*\*(?=\S)([\s\S]*?\S)\*\*(?!\*)/,em:/^_(?=\S)([\s\S]*?\S)_(?!_)|^\*(?=\S)([\s\S]*?\S)\*(?!\*)/});inline.gfm=merge({},inline.normal,{escape:replace(inline.escape)("])","~|])")(),url:/^(https?:\/\/[^\s<]+[^<.,:;"')\]\s])/,del:/^~~(?=\S)([\s\S]*?\S)~~/,text:replace(inline.text)("]|","~]|")("|","|https?://|")()});inline.breaks=merge({},inline.gfm,{br:replace(inline.br)("{2,}","*")(),text:replace(inline.gfm.text)("{2,}","*")()});function InlineLexer(links,options){this.options=options||marked.defaults;this.links=links;this.rules=inline.normal;this.renderer=this.options.renderer||new Renderer;this.renderer.options=this.options;if(!this.links){throw new Error("Tokens array requires a `links` property.")}if(this.options.gfm){if(this.options.breaks){this.rules=inline.breaks}else{this.rules=inline.gfm}}else if(this.options.pedantic){this.rules=inline.pedantic}}InlineLexer.rules=inline;InlineLexer.output=function(src,links,options){var inline=new InlineLexer(links,options);return inline.output(src)};InlineLexer.prototype.output=function(src){var out="",link,text,href,cap;while(src){if(cap=this.rules.escape.exec(src)){src=src.substring(cap[0].length);out+=cap[1];continue}if(cap=this.rules.autolink.exec(src)){src=src.substring(cap[0].length);if(cap[2]==="@"){text=cap[1].charAt(6)===":"?this.mangle(cap[1].substring(7)):this.mangle(cap[1]);href=this.mangle("mailto:")+text}else{text=escape(cap[1]);href=text}out+=this.renderer.link(href,null,text);continue}if(!this.inLink&&(cap=this.rules.url.exec(src))){src=src.substring(cap[0].length);text=escape(cap[1]);href=text;out+=this.renderer.link(href,null,text);continue}if(cap=this.rules.tag.exec(src)){if(!this.inLink&&/^<a /i.test(cap[0])){this.inLink=true}else if(this.inLink&&/^<\/a>/i.test(cap[0])){this.inLink=false}src=src.substring(cap[0].length);out+=this.options.sanitize?this.options.sanitizer?this.options.sanitizer(cap[0]):escape(cap[0]):cap[0];continue}if(cap=this.rules.link.exec(src)){src=src.substring(cap[0].length);this.inLink=true;out+=this.outputLink(cap,{href:cap[2],title:cap[3]});this.inLink=false;continue}if((cap=this.rules.reflink.exec(src))||(cap=this.rules.nolink.exec(src))){src=src.substring(cap[0].length);link=(cap[2]||cap[1]).replace(/\s+/g," ");link=this.links[link.toLowerCase()];if(!link||!link.href){out+=cap[0].charAt(0);src=cap[0].substring(1)+src;continue}this.inLink=true;out+=this.outputLink(cap,link);this.inLink=false;continue}if(cap=this.rules.strong.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.strong(this.output(cap[2]||cap[1]));continue}if(cap=this.rules.em.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.em(this.output(cap[2]||cap[1]));continue}if(cap=this.rules.code.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.codespan(escape(cap[2],true));continue}if(cap=this.rules.br.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.br();continue}if(cap=this.rules.del.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.del(this.output(cap[1]));continue}if(cap=this.rules.text.exec(src)){src=src.substring(cap[0].length);out+=this.renderer.text(escape(this.smartypants(cap[0])));continue}if(src){throw new Error("Infinite loop on byte: "+src.charCodeAt(0))}}return out};InlineLexer.prototype.outputLink=function(cap,link){var href=escape(link.href),title=link.title?escape(link.title):null;return cap[0].charAt(0)!=="!"?this.renderer.link(href,title,this.output(cap[1])):this.renderer.image(href,title,escape(cap[1]))};InlineLexer.prototype.smartypants=function(text){if(!this.options.smartypants)return text;return text.replace(/---/g,"—").replace(/--/g,"–").replace(/(^|[-\u2014\/(\[{"\s])'/g,"$1‘").replace(/'/g,"’").replace(/(^|[-\u2014\/(\[{\u2018\s])"/g,"$1“").replace(/"/g,"”").replace(/\.{3}/g,"…")};InlineLexer.prototype.mangle=function(text){if(!this.options.mangle)return text;var out="",l=text.length,i=0,ch;for(;i<l;i++){ch=text.charCodeAt(i);if(Math.random()>.5){ch="x"+ch.toString(16)}out+="&#"+ch+";"}return out};function Renderer(options){this.options=options||{}}Renderer.prototype.code=function(code,lang,escaped){if(this.options.highlight){var out=this.options.highlight(code,lang);if(out!=null&&out!==code){escaped=true;code=out}}if(!lang){return"<pre><code>"+(escaped?code:escape(code,true))+"\n</code></pre>"}return'<pre><code class="'+this.options.langPrefix+escape(lang,true)+'">'+(escaped?code:escape(code,true))+"\n</code></pre>\n"};Renderer.prototype.blockquote=function(quote){return"<blockquote>\n"+quote+"</blockquote>\n"};Renderer.prototype.html=function(html){return html};Renderer.prototype.heading=function(text,level,raw){return"<h"+level+' id="'+this.options.headerPrefix+raw.toLowerCase().replace(/[^\w]+/g,"-")+'">'+text+"</h"+level+">\n"};Renderer.prototype.hr=function(){return this.options.xhtml?"<hr/>\n":"<hr>\n"};Renderer.prototype.list=function(body,ordered){var type=ordered?"ol":"ul";return"<"+type+">\n"+body+"</"+type+">\n"};Renderer.prototype.listitem=function(text){return"<li>"+text+"</li>\n"};Renderer.prototype.paragraph=function(text){return"<p>"+text+"</p>\n"};Renderer.prototype.table=function(header,body){return"<table>\n"+"<thead>\n"+header+"</thead>\n"+"<tbody>\n"+body+"</tbody>\n"+"</table>\n"};Renderer.prototype.tablerow=function(content){return"<tr>\n"+content+"</tr>\n"};Renderer.prototype.tablecell=function(content,flags){var type=flags.header?"th":"td";var tag=flags.align?"<"+type+' style="text-align:'+flags.align+'">':"<"+type+">";return tag+content+"</"+type+">\n"};Renderer.prototype.strong=function(text){return"<strong>"+text+"</strong>"};Renderer.prototype.em=function(text){return"<em>"+text+"</em>"};Renderer.prototype.codespan=function(text){return"<code>"+text+"</code>"};Renderer.prototype.br=function(){return this.options.xhtml?"<br/>":"<br>"};Renderer.prototype.del=function(text){return"<del>"+text+"</del>"};Renderer.prototype.link=function(href,title,text){if(this.options.sanitize){try{var prot=decodeURIComponent(unescape(href)).replace(/[^\w:]/g,"").toLowerCase()}catch(e){return""}if(prot.indexOf("javascript:")===0||prot.indexOf("vbscript:")===0||prot.indexOf("data:")===0){return""}}var out='<a href="'+href+'"';if(title){out+=' title="'+title+'"'}out+=">"+text+"</a>";return out};Renderer.prototype.image=function(href,title,text){var out='<img src="'+href+'" alt="'+text+'"';if(title){out+=' title="'+title+'"'}out+=this.options.xhtml?"/>":">";return out};Renderer.prototype.text=function(text){return text};function Parser(options){this.tokens=[];this.token=null;this.options=options||marked.defaults;this.options.renderer=this.options.renderer||new Renderer;this.renderer=this.options.renderer;this.renderer.options=this.options}Parser.parse=function(src,options,renderer){var parser=new Parser(options,renderer);return parser.parse(src)};Parser.prototype.parse=function(src){this.inline=new InlineLexer(src.links,this.options,this.renderer);this.tokens=src.reverse();var out="";while(this.next()){out+=this.tok()}return out};Parser.prototype.next=function(){return this.token=this.tokens.pop()};Parser.prototype.peek=function(){return this.tokens[this.tokens.length-1]||0};Parser.prototype.parseText=function(){var body=this.token.text;while(this.peek().type==="text"){body+="\n"+this.next().text}return this.inline.output(body)};Parser.prototype.tok=function(){switch(this.token.type){case"space":{return""}case"hr":{return this.renderer.hr()}case"heading":{return this.renderer.heading(this.inline.output(this.token.text),this.token.depth,this.token.text)}case"code":{return this.renderer.code(this.token.text,this.token.lang,this.token.escaped)}case"table":{var header="",body="",i,row,cell,flags,j;cell="";for(i=0;i<this.token.header.length;i++){flags={header:true,align:this.token.align[i]};cell+=this.renderer.tablecell(this.inline.output(this.token.header[i]),{header:true,align:this.token.align[i]})}header+=this.renderer.tablerow(cell);for(i=0;i<this.token.cells.length;i++){row=this.token.cells[i];cell="";for(j=0;j<row.length;j++){cell+=this.renderer.tablecell(this.inline.output(row[j]),{header:false,align:this.token.align[j]})}body+=this.renderer.tablerow(cell)}return this.renderer.table(header,body)}case"blockquote_start":{var body="";while(this.next().type!=="blockquote_end"){body+=this.tok()}return this.renderer.blockquote(body)}case"list_start":{var body="",ordered=this.token.ordered;while(this.next().type!=="list_end"){body+=this.tok()}return this.renderer.list(body,ordered)}case"list_item_start":{var body="";while(this.next().type!=="list_item_end"){body+=this.token.type==="text"?this.parseText():this.tok()}return this.renderer.listitem(body)}case"loose_item_start":{var body="";while(this.next().type!=="list_item_end"){body+=this.tok()}return this.renderer.listitem(body)}case"html":{var html=!this.token.pre&&!this.options.pedantic?this.inline.output(this.token.text):this.token.text;return this.renderer.html(html)}case"paragraph":{return this.renderer.paragraph(this.inline.output(this.token.text))}case"text":{return this.renderer.paragraph(this.parseText())}}};function escape(html,encode){return html.replace(!encode?/&(?!#?\w+;)/g:/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;").replace(/'/g,"&#39;")}function unescape(html){return html.replace(/&(#(?:\d+)|(?:#x[0-9A-Fa-f]+)|(?:\w+));?/g,function(_,n){n=n.toLowerCase();if(n==="colon")return":";if(n.charAt(0)==="#"){return n.charAt(1)==="x"?String.fromCharCode(parseInt(n.substring(2),16)):String.fromCharCode(+n.substring(1))}return""})}function replace(regex,opt){regex=regex.source;opt=opt||"";return function self(name,val){if(!name)return new RegExp(regex,opt);val=val.source||val;val=val.replace(/(^|[^\[])\^/g,"$1");regex=regex.replace(name,val);return self}}function noop(){}noop.exec=noop;function merge(obj){var i=1,target,key;for(;i<arguments.length;i++){target=arguments[i];for(key in target){if(Object.prototype.hasOwnProperty.call(target,key)){obj[key]=target[key]}}}return obj}function marked(src,opt,callback){if(callback||typeof opt==="function"){if(!callback){callback=opt;opt=null}opt=merge({},marked.defaults,opt||{});var highlight=opt.highlight,tokens,pending,i=0;try{tokens=Lexer.lex(src,opt)}catch(e){return callback(e)}pending=tokens.length;var done=function(err){if(err){opt.highlight=highlight;return callback(err)}var out;try{out=Parser.parse(tokens,opt)}catch(e){err=e}opt.highlight=highlight;return err?callback(err):callback(null,out)};if(!highlight||highlight.length<3){return done()}delete opt.highlight;if(!pending)return done();for(;i<tokens.length;i++){(function(token){if(token.type!=="code"){return--pending||done()}return highlight(token.text,token.lang,function(err,code){if(err)return done(err);if(code==null||code===token.text){return--pending||done()}token.text=code;token.escaped=true;--pending||done()})})(tokens[i])}return}try{if(opt)opt=merge({},marked.defaults,opt);return Parser.parse(Lexer.lex(src,opt),opt)}catch(e){e.message+="\nPlease report this to https://github.com/chjj/marked.";if((opt||marked.defaults).silent){return"<p>An error occured:</p><pre>"+escape(e.message+"",true)+"</pre>"}throw e}}marked.options=marked.setOptions=function(opt){merge(marked.defaults,opt);return marked};marked.defaults={gfm:true,tables:true,breaks:false,pedantic:false,sanitize:false,sanitizer:null,mangle:true,smartLists:false,silent:false,highlight:null,langPrefix:"lang-",smartypants:false,headerPrefix:"",renderer:new Renderer,xhtml:false};marked.Parser=Parser;marked.parser=Parser.parse;marked.Renderer=Renderer;marked.Lexer=Lexer;marked.lexer=Lexer.lex;marked.InlineLexer=InlineLexer;marked.inlineLexer=InlineLexer.output;marked.parse=marked;if(typeof module!=="undefined"&&typeof exports==="object"){module.exports=marked}else if(typeof define==="function"&&define.amd){define(function(){return marked})}else{this.marked=marked}}).call(function(){return this||(typeof window!=="undefined"?window:global)}());

	return module.exports;
}();


// FORMAT OPTIONS FOR MARKED IMPLEMENTATION

function _Markdown_formatOptions(options)
{
	function toHighlight(code, lang)
	{
		if (!lang && $elm$core$Maybe$isJust(options.X))
		{
			lang = options.X.a;
		}

		if (typeof hljs !== 'undefined' && lang && hljs.listLanguages().indexOf(lang) >= 0)
		{
			return hljs.highlight(lang, code, true).value;
		}

		return code;
	}

	var gfm = options.ah.a;

	return {
		highlight: toHighlight,
		gfm: gfm,
		tables: gfm && gfm.ap,
		breaks: gfm && gfm.ac,
		sanitize: options.an,
		smartypants: options.ao
	};
}

var $elm$core$Basics$add=_Basics_add;
var $elm$core$Basics$sub=_Basics_sub;
var $elm$core$Basics$mul=_Basics_mul;
var $elm$core$Basics$fdiv=_Basics_fdiv;
var $elm$core$Basics$idiv=_Basics_idiv;
var $elm$core$Basics$toFloat=_Basics_toFloat;
var $elm$core$Basics$floor=_Basics_floor;
var $elm$core$Basics$ceiling=_Basics_ceiling;
var $elm$core$Basics$eq=_Utils_equal;
var $elm$core$Basics$neq=_Utils_notEqual;
var $elm$core$Basics$lt=_Utils_lt;
var $elm$core$Basics$gt=_Utils_gt;
var $elm$core$Basics$le=_Utils_le;
var $elm$core$Basics$ge=_Utils_ge;
var $elm$core$Basics$max=F2(function($l2,$l3){if((_Utils_cmp($l2,$l3)>0)){return $l2;}else{return $l3;};});
var $elm$core$Basics$compare=_Utils_compare;
var $elm$core$Basics$LT=0;
var $elm$core$Basics$EQ=1;
var $elm$core$Basics$GT=2;
var $elm$core$Basics$not=_Basics_not;
var $elm$core$Basics$and=_Basics_and;
var $elm$core$Basics$or=_Basics_or;
var $elm$core$Basics$append=_Utils_append;
var $elm$core$Basics$remainderBy=_Basics_remainderBy;
var $elm$core$Basics$logBase=F2(function($l9,$l10){return ((_Basics_log)($l10) / (_Basics_log)($l9));;});
var $elm$core$Basics$composeL=F3(function($l18,$l19,$l20){return ($l18)(($l19)($l20));;});
var $elm$core$Basics$apR=F2(function($l24,$l25){return ($l25)($l24);;});
var $elm$core$Basics$apL=F2(function($l26,$l27){return ($l26)($l27);;});
var $elm$core$Basics$identity=(function($l28){return $l28;;});
var $elm$core$Basics$always=F2(function($l29,$tailInput1){return $l29;;});
var $elm$core$Basics$never=(function($tailInput0){let $tailState0=$tailInput0;$tailLoop:while(true){var $l30=$tailState0;let $tailNext0=$l30;$tailState0=$tailNext0;continue $tailLoop;};});
var $elm$core$Bitwise$and=_Bitwise_and;
var $elm$core$Bitwise$shiftLeftBy=_Bitwise_shiftLeftBy;
var $elm$core$Bitwise$shiftRightBy=_Bitwise_shiftRightBy;
var $elm$core$Bitwise$shiftRightZfBy=_Bitwise_shiftRightZfBy;
var $elm$core$Elm$JsArray$empty=_JsArray_empty;
var $elm$core$Elm$JsArray$singleton=_JsArray_singleton;
var $elm$core$Elm$JsArray$length=_JsArray_length;
var $elm$core$Elm$JsArray$initialize=_JsArray_initialize;
var $elm$core$Elm$JsArray$initializeFromList=_JsArray_initializeFromList;
var $elm$core$Elm$JsArray$unsafeGet=_JsArray_unsafeGet;
var $elm$core$Elm$JsArray$unsafeSet=_JsArray_unsafeSet;
var $elm$core$Elm$JsArray$push=_JsArray_push;
var $elm$core$Elm$JsArray$foldl=_JsArray_foldl;
var $elm$core$Elm$JsArray$foldr=_JsArray_foldr;
var $elm$core$Maybe$Just=function(c0){return {$:0,a:c0};};
var $elm$core$Maybe$Nothing=({$:1});
var $elm$core$Maybe$withDefault=F2(function($l0,$l1){let $tailCase3=$l1;if($tailCase3.$===0){{var $l2=$tailCase3.a;return $l2;}}{return $l0;};});
var $elm$core$Maybe$map4=F5(function($l18,$l19,$l20,$l21,$l22){let $tailCase59=$l19;if($tailCase59.$===1){{return $elm$core$Maybe$Nothing;}}{var $l23=$tailCase59.a;let $tailCase58=$l20;if($tailCase58.$===1){{return $elm$core$Maybe$Nothing;}}{var $l24=$tailCase58.a;let $tailCase57=$l21;if($tailCase57.$===1){{return $elm$core$Maybe$Nothing;}}{var $l25=$tailCase57.a;let $tailCase56=$l22;if($tailCase56.$===1){{return $elm$core$Maybe$Nothing;}}{var $l26=$tailCase56.a;return ($elm$core$Maybe$Just)(A4($l18,$l23,$l24,$l25,$l26));}}}};});
var $elm$core$Maybe$andThen=F2(function($l38,$l39){let $tailCase89=$l39;if($tailCase89.$===0){{var $l40=$tailCase89.a;return ($l38)($l40);}}{return $elm$core$Maybe$Nothing;};});
var $elm$core$Maybe$isJust=(function($l41){let $tailCase93=$l41;if($tailCase93.$===0){{return true;}}{return false;};});
var $elm$core$List$cons=_List_cons;
var $elm$core$List$rangeHelp=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l8=$tailState0;var $l9=$tailState1;var $l10=$tailState2;if((_Utils_cmp($l8,$l9)<1)){let $tailNext0=$l8;let $tailNext1=($l9 - 1);let $tailNext2=A2($elm$core$List$cons,$l9,$l10);$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}else{return $l10;}};});
var $elm$core$List$range=F2(function($l6,$l7){return A3($elm$core$List$rangeHelp,$l6,$l7,_List_Nil);;});
var $elm$core$List$foldl=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l17=$tailState0;var $l18=$tailState1;var $l19=$tailState2;let $tailCase76=$l19;if(($tailCase76.$===0)){{return $l18;}}{var $l20=$tailCase76.a;var $l21=$tailCase76.b;let $tailNext0=$l17;let $tailNext1=A2($l17,$l20,$l18);let $tailNext2=$l21;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}};});
var $elm$core$List$reverse=(function($l50){return A3($elm$core$List$foldl,$elm$core$List$cons,_List_Nil,$l50);;});
var $elm$core$List$foldrHelper=F4(function($l25,$l26,$l27,$l28){let $tailCase145=$l28;if(($tailCase145.$===0)){{return $l26;}}{var $l29=$tailCase145.a;var $l30=$tailCase145.b;let $tailCase144=$l30;if(($tailCase144.$===0)){{return A2($l25,$l29,$l26);}}{var $l31=$tailCase144.a;var $l32=$tailCase144.b;let $tailCase143=$l32;if(($tailCase143.$===0)){{return A2($l25,$l29,A2($l25,$l31,$l26));}}{var $l33=$tailCase143.a;var $l34=$tailCase143.b;let $tailCase142=$l34;if(($tailCase142.$===0)){{return A2($l25,$l29,A2($l25,$l31,A2($l25,$l33,$l26)));}}{var $l35=$tailCase142.a;var $l36=$tailCase142.b;{var $l37=((_Utils_cmp($l27,500)>0)?A3($elm$core$List$foldl,$l25,$l26,($elm$core$List$reverse)($l36)):A4($elm$core$List$foldrHelper,$l25,$l26,($l27 + 1),$l36));return A2($l25,$l29,A2($l25,$l31,A2($l25,$l33,A2($l25,$l35,$l37))));}}}}};});
var $elm$core$List$foldr=F3(function($l22,$l23,$l24){return A4($elm$core$List$foldrHelper,$l22,$l23,0,$l24);;});
var $elm$core$List$map=F2(function($l11,$l12){return A3($elm$core$List$foldr,F2(function($arg50_0,$arg50_1){var $l13=$arg50_0;var $l14=$arg50_1;return A2($elm$core$List$cons,($l11)($l13),$l14);}),_List_Nil,$l12);;});
var $elm$core$List$map2=_List_map2;
var $elm$core$List$length=(function($l48){return A3($elm$core$List$foldl,F2(function($arg180_0,$arg180_1){var $l49=$arg180_1;return ($l49 + 1);}),0,$l48);;});
var $elm$core$List$indexedMap=F2(function($l15,$l16){return A3($elm$core$List$map2,$l15,A2($elm$core$List$range,0,(($elm$core$List$length)($l16) - 1)),$l16);;});
var $elm$core$List$maybeCons=F3(function($l44,$l45,$l46){let $tailCase175=($l44)($l45);if($tailCase175.$===0){{var $l47=$tailCase175.a;return A2($elm$core$List$cons,$l47,$l46);}}{return $l46;};});
var $elm$core$List$filterMap=F2(function($l42,$l43){return A3($elm$core$List$foldr,($elm$core$List$maybeCons)($l42),_List_Nil,$l43);;});
var $elm$core$List$any=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l56=$tailState0;var $l57=$tailState1;let $tailCase215=$l57;if(($tailCase215.$===0)){{return false;}}{var $l58=$tailCase215.a;var $l59=$tailCase215.b;if(($l56)($l58)){return true;}else{let $tailNext0=$l56;let $tailNext1=$l59;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}};});
var $elm$core$List$all=F2(function($l54,$l55){return (!A2($elm$core$List$any,A2($elm$core$Basics$composeL,$elm$core$Basics$not,$l54),$l55));;});
var $elm$core$List$append=F2(function($l68,$l69){let $tailCase253=$l69;if(($tailCase253.$===0)){{return $l68;}}{return A3($elm$core$List$foldr,$elm$core$List$cons,$l69,$l68);};});
var $elm$core$List$concat=(function($l70){return A3($elm$core$List$foldr,$elm$core$List$append,_List_Nil,$l70);;});
var $elm$core$List$concatMap=F2(function($l71,$l72){return ($elm$core$List$concat)(A2($elm$core$List$map,$l71,$l72));;});
var $elm$core$List$intersperse=F2(function($l73,$l74){let $tailCase284=$l74;if(($tailCase284.$===0)){{return _List_Nil;}}{var $l75=$tailCase284.a;var $l76=$tailCase284.b;{var $l77=F2(function($l79,$l80){return A2($elm$core$List$cons,$l73,A2($elm$core$List$cons,$l79,$l80));;});var $l78=A3($elm$core$List$foldr,$l77,_List_Nil,$l76);return A2($elm$core$List$cons,$l75,$l78);}};});
var $elm$core$List$sortBy=_List_sortBy;
var $elm$core$List$sort=(function($l81){return A2($elm$core$List$sortBy,$elm$core$Basics$identity,$l81);;});
var $elm$core$List$isEmpty=(function($l82){let $tailCase298=$l82;if(($tailCase298.$===0)){{return true;}}{return false;};});
var $elm$core$Tuple$first=(function($tailInput0){var $l2=$tailInput0.a;return $l2;;});
var $elm$core$Tuple$second=(function($tailInput0){var $l3=$tailInput0.b;return $l3;;});
var $elm$core$Array$branchFactor=32;
var $elm$core$Array$shiftStep=($elm$core$Basics$ceiling)(A2($elm$core$Basics$logBase,2,($elm$core$Array$branchFactor)));
var $elm$core$Array$bitMask=A2($elm$core$Bitwise$shiftRightZfBy,(32 - $elm$core$Array$shiftStep),0xFFFFFFFF);
var $elm$core$Array$Array_elm_builtin=F4(function(c0,c1,c2,c3){return {$:0,a:c0,b:c1,c:c2,d:c3};});
var $elm$core$Array$SubTree=function(c0){return {$:0,a:c0};};
var $elm$core$Array$Leaf=function(c0){return {$:1,a:c0};};
var $elm$core$Array$empty=A4($elm$core$Array$Array_elm_builtin,0,$elm$core$Array$shiftStep,$elm$core$Elm$JsArray$empty,$elm$core$Elm$JsArray$empty);
var $elm$core$Array$length=(function($tailInput0){var $l1=$tailInput0.a;return $l1;;});
var $elm$core$Array$compressNodes=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l220=$tailState0;var $l221=$tailState1;{var $tailDestruct1165_0=A2($elm$core$Elm$JsArray$initializeFromList,$elm$core$Array$branchFactor,$l220);var $l222=$tailDestruct1165_0.a;var $l223=$tailDestruct1165_0.b;var $l224=A2($elm$core$List$cons,($elm$core$Array$SubTree)($l222),$l221);let $tailCase1164=$l223;if(($tailCase1164.$===0)){{return ($elm$core$List$reverse)($l224);}}{let $tailNext0=$l223;let $tailNext1=$l224;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}};});
var $elm$core$Array$treeFromBuilder=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l217=$tailState0;var $l218=$tailState1;{var $l219=($elm$core$Basics$ceiling)((($l218) / ($elm$core$Array$branchFactor)));if(_Utils_eq($l219,1)){return ($elm$core$Tuple$first)(A2($elm$core$Elm$JsArray$initializeFromList,$elm$core$Array$branchFactor,$l217));}else{let $tailNext0=A2($elm$core$Array$compressNodes,$l217,_List_Nil);let $tailNext1=$l219;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}};});
var $elm$core$Array$builderToArray=F2(function($l211,$l212){if(_Utils_eq(($l212)["c"],0)){return A4($elm$core$Array$Array_elm_builtin,($elm$core$Elm$JsArray$length)(($l212)["e"]),$elm$core$Array$shiftStep,$elm$core$Elm$JsArray$empty,($l212)["e"]);}else{{var $l213=(($l212)["c"] * $elm$core$Array$branchFactor);var $l214=($elm$core$Basics$floor)(A2($elm$core$Basics$logBase,($elm$core$Array$branchFactor),(($l213 - 1))));var $l215=($l211?($elm$core$List$reverse)(($l212)["g"]):($l212)["g"]);var $l216=A2($elm$core$Array$treeFromBuilder,$l215,($l212)["c"]);return A4($elm$core$Array$Array_elm_builtin,(($elm$core$Elm$JsArray$length)(($l212)["e"]) + $l213),A2($elm$core$Basics$max,5,($l214 * $elm$core$Array$shiftStep)),$l216,($l212)["e"]);}};});
var $elm$core$Array$initializeHelp=F5(function($tailInput0,$tailInput1,$tailInput2,$tailInput3,$tailInput4){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;let $tailState3=$tailInput3;let $tailState4=$tailInput4;$tailLoop:while(true){var $l7=$tailState0;var $l8=$tailState1;var $l9=$tailState2;var $l10=$tailState3;var $l11=$tailState4;if((_Utils_cmp($l8,0)<0)){return A2($elm$core$Array$builderToArray,false,({"g":$l10,"c":(($l9)/($elm$core$Array$branchFactor)|0),"e":$l11}));}else{{var $l12=($elm$core$Array$Leaf)(A3($elm$core$Elm$JsArray$initialize,$elm$core$Array$branchFactor,$l8,$l7));let $tailNext0=$l7;let $tailNext1=($l8 - $elm$core$Array$branchFactor);let $tailNext2=$l9;let $tailNext3=A2($elm$core$List$cons,$l12,$l10);let $tailNext4=$l11;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;$tailState3=$tailNext3;$tailState4=$tailNext4;continue $tailLoop;}}};});
var $elm$core$Array$initialize=F2(function($l2,$l3){if((_Utils_cmp($l2,0)<1)){return $elm$core$Array$empty;}else{{var $l4=A2($elm$core$Basics$remainderBy,$elm$core$Array$branchFactor,$l2);var $l5=A3($elm$core$Elm$JsArray$initialize,$l4,($l2 - $l4),$l3);var $l6=(($l2 - $l4) - $elm$core$Array$branchFactor);return A5($elm$core$Array$initializeHelp,$l3,$l6,$l2,_List_Nil,$l5);}};});
var $elm$core$Array$fromListHelp=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l16=$tailState0;var $l17=$tailState1;var $l18=$tailState2;{var $tailDestruct126_0=A2($elm$core$Elm$JsArray$initializeFromList,$elm$core$Array$branchFactor,$l16);var $l19=$tailDestruct126_0.a;var $l20=$tailDestruct126_0.b;if((_Utils_cmp(($elm$core$Elm$JsArray$length)($l19),$elm$core$Array$branchFactor)<0)){return A2($elm$core$Array$builderToArray,true,({"g":$l17,"c":$l18,"e":$l19}));}else{let $tailNext0=$l20;let $tailNext1=A2($elm$core$List$cons,($elm$core$Array$Leaf)($l19),$l17);let $tailNext2=($l18 + 1);$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}}};});
var $elm$core$Array$fromList=(function($l15){let $tailCase97=$l15;if(($tailCase97.$===0)){{return $elm$core$Array$empty;}}{return A3($elm$core$Array$fromListHelp,$l15,_List_Nil,0);};});
var $elm$core$Array$tailIndex=(function($l32){return A2($elm$core$Bitwise$shiftLeftBy,5,A2($elm$core$Bitwise$shiftRightZfBy,5,$l32));;});
var $elm$core$Array$getHelp=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l26=$tailState0;var $l27=$tailState1;var $l28=$tailState2;{var $l29=A2($elm$core$Bitwise$and,$elm$core$Array$bitMask,A2($elm$core$Bitwise$shiftRightZfBy,$l26,$l27));let $tailCase182=A2($elm$core$Elm$JsArray$unsafeGet,$l29,$l28);if($tailCase182.$===0){{var $l30=$tailCase182.a;let $tailNext0=($l26 - $elm$core$Array$shiftStep);let $tailNext1=$l27;let $tailNext2=$l30;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}}{var $l31=$tailCase182.a;return A2($elm$core$Elm$JsArray$unsafeGet,A2($elm$core$Bitwise$and,$elm$core$Array$bitMask,$l27),$l31);}}};});
var $elm$core$Array$get=F2(function($l21,$tailInput1){var $l22=$tailInput1.a;var $l23=$tailInput1.b;var $l24=$tailInput1.c;var $l25=$tailInput1.d;if(((_Utils_cmp($l21,0)<0)||(_Utils_cmp($l21,$l22)>-1))){return $elm$core$Maybe$Nothing;}else{if((_Utils_cmp($l21,($elm$core$Array$tailIndex)($l22))>-1)){return ($elm$core$Maybe$Just)(A2($elm$core$Elm$JsArray$unsafeGet,A2($elm$core$Bitwise$and,$elm$core$Array$bitMask,$l21),$l25));}else{return ($elm$core$Maybe$Just)(A3($elm$core$Array$getHelp,$l23,$l21,$l24));}};});
var $elm$core$Array$insertTailInTree=F4(function($l63,$l64,$l65,$l66){{var $l67=A2($elm$core$Bitwise$and,$elm$core$Array$bitMask,A2($elm$core$Bitwise$shiftRightZfBy,$l63,$l64));if((_Utils_cmp($l67,($elm$core$Elm$JsArray$length)($l66))>-1)){if(_Utils_eq($l63,5)){return A2($elm$core$Elm$JsArray$push,($elm$core$Array$Leaf)($l65),$l66);}else{{var $l68=($elm$core$Array$SubTree)(A4($elm$core$Array$insertTailInTree,($l63 - $elm$core$Array$shiftStep),$l64,$l65,$elm$core$Elm$JsArray$empty));return A2($elm$core$Elm$JsArray$push,$l68,$l66);}}}else{{var $l69=A2($elm$core$Elm$JsArray$unsafeGet,$l67,$l66);let $tailCase426=$l69;if($tailCase426.$===0){{var $l70=$tailCase426.a;{var $l71=($elm$core$Array$SubTree)(A4($elm$core$Array$insertTailInTree,($l63 - $elm$core$Array$shiftStep),$l64,$l65,$l70));return A3($elm$core$Elm$JsArray$unsafeSet,$l67,$l71,$l66);}}}{{var $l72=($elm$core$Array$SubTree)(A4($elm$core$Array$insertTailInTree,($l63 - $elm$core$Array$shiftStep),$l64,$l65,($elm$core$Elm$JsArray$singleton)($l69)));return A3($elm$core$Elm$JsArray$unsafeSet,$l67,$l72,$l66);}}}}};});
var $elm$core$Array$unsafeReplaceTail=F2(function($l52,$tailInput1){var $l53=$tailInput1.a;var $l54=$tailInput1.b;var $l55=$tailInput1.c;var $l56=$tailInput1.d;{var $l57=($elm$core$Elm$JsArray$length)($l56);var $l58=($elm$core$Elm$JsArray$length)($l52);var $l59=($l53 + ($l58 - $l57));if(_Utils_eq($l58,$elm$core$Array$branchFactor)){{var $l60=(_Utils_cmp(A2($elm$core$Bitwise$shiftRightZfBy,$elm$core$Array$shiftStep,$l59),A2($elm$core$Bitwise$shiftLeftBy,$l54,1))>0);if($l60){{var $l61=($l54 + $elm$core$Array$shiftStep);var $l62=A4($elm$core$Array$insertTailInTree,$l61,$l53,$l52,($elm$core$Elm$JsArray$singleton)(($elm$core$Array$SubTree)($l55)));return A4($elm$core$Array$Array_elm_builtin,$l59,$l61,$l62,$elm$core$Elm$JsArray$empty);}}else{return A4($elm$core$Array$Array_elm_builtin,$l59,$l54,A4($elm$core$Array$insertTailInTree,$l54,$l53,$l52,$l55),$elm$core$Elm$JsArray$empty);}}}else{return A4($elm$core$Array$Array_elm_builtin,$l59,$l54,$l55,$l52);}};});
var $elm$core$Array$push=F2(function($l49,$tailInput1){var $l50=$tailInput1;var $l51=$tailInput1.d;return A2($elm$core$Array$unsafeReplaceTail,A2($elm$core$Elm$JsArray$push,$l49,$l51),$l50);;});
var $elm$core$Array$foldr=F3(function($l80,$l81,$tailInput2){var $l82=$tailInput2.c;var $l83=$tailInput2.d;{var $l84=F2(function($l85,$l86){let $tailCase467=$l85;if($tailCase467.$===0){{var $l87=$tailCase467.a;return A3($elm$core$Elm$JsArray$foldr,$l84,$l86,$l87);}}{var $l88=$tailCase467.a;return A3($elm$core$Elm$JsArray$foldr,$l80,$l86,$l88);};});return A3($elm$core$Elm$JsArray$foldr,$l84,A3($elm$core$Elm$JsArray$foldr,$l80,$l81,$l83),$l82);};});
var $elm$core$Array$toList=(function($l73){return A3($elm$core$Array$foldr,$elm$core$List$cons,_List_Nil,$l73);;});
var $elm$core$Array$foldl=F3(function($l89,$l90,$tailInput2){var $l91=$tailInput2.c;var $l92=$tailInput2.d;{var $l93=F2(function($l94,$l95){let $tailCase489=$l94;if($tailCase489.$===0){{var $l96=$tailCase489.a;return A3($elm$core$Elm$JsArray$foldl,$l93,$l95,$l96);}}{var $l97=$tailCase489.a;return A3($elm$core$Elm$JsArray$foldl,$l89,$l95,$l97);};});return A3($elm$core$Elm$JsArray$foldl,$l89,A3($elm$core$Elm$JsArray$foldl,$l93,$l90,$l91),$l92);};});
var $elm$core$Char$toCode=_Char_toCode;
var $elm$core$Char$isUpper=(function($l0){{var $l1=($elm$core$Char$toCode)($l0);return ((_Utils_cmp($l1,0x5A)<1)&&(_Utils_cmp(0x41,$l1)<1));};});
var $elm$core$Char$isLower=(function($l2){{var $l3=($elm$core$Char$toCode)($l2);return ((_Utils_cmp(0x61,$l3)<1)&&(_Utils_cmp($l3,0x7A)<1));};});
var $elm$core$Char$isAlpha=(function($l4){return (($elm$core$Char$isLower)($l4)||($elm$core$Char$isUpper)($l4));;});
var $elm$core$Char$isDigit=(function($l6){{var $l7=($elm$core$Char$toCode)($l6);return ((_Utils_cmp($l7,0x39)<1)&&(_Utils_cmp(0x30,$l7)<1));};});
var $elm$core$Char$isAlphaNum=(function($l5){return (($elm$core$Char$isLower)($l5)||(($elm$core$Char$isUpper)($l5)||($elm$core$Char$isDigit)($l5)));;});
var $elm$core$Result$Ok=function(c0){return {$:0,a:c0};};
var $elm$core$Result$Err=function(c0){return {$:1,a:c0};};
var $elm$core$Result$isOk=(function($l66){let $tailCase149=$l66;if($tailCase149.$===0){{return true;}}{return false;};});
var $elm$core$String$isEmpty=(function($l0){return _Utils_eq($l0,'');;});
var $elm$core$String$length=_String_length;
var $elm$core$String$repeatHelp=F3(function($l3,$l4,$l5){if((_Utils_cmp($l3,0)<1)){return $l5;}else{return A3($elm$core$String$repeatHelp,A2($elm$core$Bitwise$shiftRightBy,1,$l3),_Utils_ap($l4,$l4),(_Utils_eq(A2($elm$core$Bitwise$and,$l3,1),0)?$l5:_Utils_ap($l5,$l4)));};});
var $elm$core$String$repeat=F2(function($l1,$l2){return A3($elm$core$String$repeatHelp,$l1,$l2,'');;});
var $elm$core$String$split=F2(function($l10,$l11){return (_List_fromArray)(A2(_String_split,$l10,$l11));;});
var $elm$core$String$join=F2(function($l12,$l13){return A2(_String_join,$l12,(_List_toArray)($l13));;});
var $elm$core$String$slice=_String_slice;
var $elm$core$String$left=F2(function($l14,$l15){if((_Utils_cmp($l14,1)<0)){return '';}else{return A3($elm$core$String$slice,0,$l14,$l15);};});
var $elm$core$String$right=F2(function($l16,$l17){if((_Utils_cmp($l16,1)<0)){return '';}else{return A3($elm$core$String$slice,(-$l16),($elm$core$String$length)($l17),$l17);};});
var $elm$core$String$dropLeft=F2(function($l18,$l19){if((_Utils_cmp($l18,1)<0)){return $l19;}else{return A3($elm$core$String$slice,$l18,($elm$core$String$length)($l19),$l19);};});
var $elm$core$String$contains=_String_contains;
var $elm$core$String$startsWith=_String_startsWith;
var $elm$core$String$indexes=_String_indexes;
var $elm$core$String$toLower=_String_toLower;
var $elm$core$String$cons=_String_cons;
var $elm$core$String$fromChar=(function($l33){return A2($elm$core$String$cons,$l33,'');;});
var $elm$core$String$padLeft=F3(function($l26,$l27,$l28){return _Utils_ap(A2($elm$core$String$repeat,($l26 - ($elm$core$String$length)($l28)),($elm$core$String$fromChar)($l27)),$l28);;});
var $elm$core$String$toInt=_String_toInt;
var $elm$core$String$fromInt=_String_fromNumber;
var $elm$core$String$uncons=_String_uncons;
var $elm$core$String$any=_String_any;
var $elm$core$String$all=_String_all;
var $elm$core$Dict$RBNode_elm_builtin=F5(function(c0,c1,c2,c3,c4){return {$:-1,a:c0,b:c1,c:c2,d:c3,e:c4};});
var $elm$core$Dict$RBEmpty_elm_builtin=({$:-2});
var $elm$core$Dict$empty=$elm$core$Dict$RBEmpty_elm_builtin;
var $elm$core$Dict$get=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l0=$tailState0;var $l1=$tailState1;let $tailCase19=$l1;if($tailCase19.$===-2){{return $elm$core$Maybe$Nothing;}}{var $l2=$tailCase19.b;var $l3=$tailCase19.c;var $l4=$tailCase19.d;var $l5=$tailCase19.e;let $tailCase18=A2($elm$core$Basics$compare,$l0,$l2);switch($tailCase18){case 0:{let $tailNext0=$l0;let $tailNext1=$l4;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}case 1:{return ($elm$core$Maybe$Just)($l3);}default:{let $tailNext0=$l0;let $tailNext1=$l5;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}}};});
var $elm$core$Dict$sizeHelp=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l9=$tailState0;var $l10=$tailState1;let $tailCase42=$l10;if($tailCase42.$===-2){{return $l9;}}{var $l11=$tailCase42.d;var $l12=$tailCase42.e;let $tailNext0=A2($elm$core$Dict$sizeHelp,($l9 + 1),$l12);let $tailNext1=$l11;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}};});
var $elm$core$Dict$size=(function($l8){return A2($elm$core$Dict$sizeHelp,0,$l8);;});
var $elm$core$Dict$isEmpty=(function($l13){let $tailCase46=$l13;if($tailCase46.$===-2){{return true;}}{return false;};});
var $elm$core$Dict$balance=F5(function($l30,$l31,$l32,$l33,$l34){let $tailCase167=$l34;if($tailCase167.$===-1&&$tailCase167.a===0){{var $l35=$tailCase167.b;var $l36=$tailCase167.c;var $l37=$tailCase167.d;var $l38=$tailCase167.e;let $tailCase138=$l33;if($tailCase138.$===-1&&$tailCase138.a===0){{var $l39=$tailCase138.b;var $l40=$tailCase138.c;var $l41=$tailCase138.d;var $l42=$tailCase138.e;return A5($elm$core$Dict$RBNode_elm_builtin,0,$l31,$l32,A5($elm$core$Dict$RBNode_elm_builtin,1,$l39,$l40,$l41,$l42),A5($elm$core$Dict$RBNode_elm_builtin,1,$l35,$l36,$l37,$l38));}}{return A5($elm$core$Dict$RBNode_elm_builtin,$l30,$l35,$l36,A5($elm$core$Dict$RBNode_elm_builtin,0,$l31,$l32,$l33,$l37),$l38);}}}{let $tailCase166=$l33;if($tailCase166.$===-1&&$tailCase166.a===0&&$tailCase166.d.$===-1&&$tailCase166.d.a===0){{var $l43=$tailCase166.b;var $l44=$tailCase166.c;var $l45=$tailCase166.d.b;var $l46=$tailCase166.d.c;var $l47=$tailCase166.d.d;var $l48=$tailCase166.d.e;var $l49=$tailCase166.e;return A5($elm$core$Dict$RBNode_elm_builtin,0,$l43,$l44,A5($elm$core$Dict$RBNode_elm_builtin,1,$l45,$l46,$l47,$l48),A5($elm$core$Dict$RBNode_elm_builtin,1,$l31,$l32,$l49,$l34));}}{return A5($elm$core$Dict$RBNode_elm_builtin,$l30,$l31,$l32,$l33,$l34);}};});
var $elm$core$Dict$insertHelp=F3(function($l22,$l23,$l24){let $tailCase103=$l24;if($tailCase103.$===-2){{return A5($elm$core$Dict$RBNode_elm_builtin,0,$l22,$l23,$elm$core$Dict$RBEmpty_elm_builtin,$elm$core$Dict$RBEmpty_elm_builtin);}}{var $l25=$tailCase103.a;var $l26=$tailCase103.b;var $l27=$tailCase103.c;var $l28=$tailCase103.d;var $l29=$tailCase103.e;let $tailCase102=A2($elm$core$Basics$compare,$l22,$l26);switch($tailCase102){case 0:{return A5($elm$core$Dict$balance,$l25,$l26,$l27,A3($elm$core$Dict$insertHelp,$l22,$l23,$l28),$l29);}case 1:{return A5($elm$core$Dict$RBNode_elm_builtin,$l25,$l26,$l23,$l28,$l29);}default:{return A5($elm$core$Dict$balance,$l25,$l26,$l27,$l28,A3($elm$core$Dict$insertHelp,$l22,$l23,$l29));}}};});
var $elm$core$Dict$insert=F3(function($l14,$l15,$l16){let $tailCase60=A3($elm$core$Dict$insertHelp,$l14,$l15,$l16);if($tailCase60.$===-1&&$tailCase60.a===0){{var $l17=$tailCase60.b;var $l18=$tailCase60.c;var $l19=$tailCase60.d;var $l20=$tailCase60.e;return A5($elm$core$Dict$RBNode_elm_builtin,1,$l17,$l18,$l19,$l20);}}{var $l21=$tailCase60;return $l21;};});
var $elm$core$Dict$moveRedLeft=(function($l105){let $tailCase406=$l105;if($tailCase406.$===-1){if($tailCase406.d.$===-1){if($tailCase406.e.$===-1){if($tailCase406.e.d.$===-1&&$tailCase406.e.d.a===0){{var $l106=$tailCase406.a;var $l107=$tailCase406.b;var $l108=$tailCase406.c;var $l109=$tailCase406.d.a;var $l110=$tailCase406.d.b;var $l111=$tailCase406.d.c;var $l112=$tailCase406.d.d;var $l113=$tailCase406.d.e;var $l114=$tailCase406.e.a;var $l115=$tailCase406.e.b;var $l116=$tailCase406.e.c;var $l117=$tailCase406.e.d;var $l118=$tailCase406.e.d.b;var $l119=$tailCase406.e.d.c;var $l120=$tailCase406.e.d.d;var $l121=$tailCase406.e.d.e;var $l122=$tailCase406.e.e;return A5($elm$core$Dict$RBNode_elm_builtin,0,$l118,$l119,A5($elm$core$Dict$RBNode_elm_builtin,1,$l107,$l108,A5($elm$core$Dict$RBNode_elm_builtin,0,$l110,$l111,$l112,$l113),$l120),A5($elm$core$Dict$RBNode_elm_builtin,1,$l115,$l116,$l121,$l122));}}{var $l123=$tailCase406.a;var $l124=$tailCase406.b;var $l125=$tailCase406.c;var $l126=$tailCase406.d.a;var $l127=$tailCase406.d.b;var $l128=$tailCase406.d.c;var $l129=$tailCase406.d.d;var $l130=$tailCase406.d.e;var $l131=$tailCase406.e.a;var $l132=$tailCase406.e.b;var $l133=$tailCase406.e.c;var $l134=$tailCase406.e.d;var $l135=$tailCase406.e.e;let $tailCase404=$l123;if($tailCase404===1){{return A5($elm$core$Dict$RBNode_elm_builtin,1,$l124,$l125,A5($elm$core$Dict$RBNode_elm_builtin,0,$l127,$l128,$l129,$l130),A5($elm$core$Dict$RBNode_elm_builtin,0,$l132,$l133,$l134,$l135));}}{return A5($elm$core$Dict$RBNode_elm_builtin,1,$l124,$l125,A5($elm$core$Dict$RBNode_elm_builtin,0,$l127,$l128,$l129,$l130),A5($elm$core$Dict$RBNode_elm_builtin,0,$l132,$l133,$l134,$l135));}}}}}{return $l105;};});
var $elm$core$Dict$removeMin=(function($l92){let $tailCase338=$l92;if($tailCase338.$===-1&&$tailCase338.d.$===-1){{var $l93=$tailCase338.a;var $l94=$tailCase338.b;var $l95=$tailCase338.c;var $l96=$tailCase338.d;var $l97=$tailCase338.d.a;var $l98=$tailCase338.d.d;var $l99=$tailCase338.e;let $tailCase336=$l97;if($tailCase336===1){{let $tailCase326=$l98;if($tailCase326.$===-1&&$tailCase326.a===0){{return A5($elm$core$Dict$RBNode_elm_builtin,$l93,$l94,$l95,($elm$core$Dict$removeMin)($l96),$l99);}}{let $tailCase325=($elm$core$Dict$moveRedLeft)($l92);if($tailCase325.$===-1){{var $l100=$tailCase325.a;var $l101=$tailCase325.b;var $l102=$tailCase325.c;var $l103=$tailCase325.d;var $l104=$tailCase325.e;return A5($elm$core$Dict$balance,$l100,$l101,$l102,($elm$core$Dict$removeMin)($l103),$l104);}}{return $elm$core$Dict$RBEmpty_elm_builtin;}}}}{return A5($elm$core$Dict$RBNode_elm_builtin,$l93,$l94,$l95,($elm$core$Dict$removeMin)($l96),$l99);}}}{return $elm$core$Dict$RBEmpty_elm_builtin;};});
var $elm$core$Dict$getMin=(function($tailInput0){let $tailState0=$tailInput0;$tailLoop:while(true){var $l90=$tailState0;let $tailCase299=$l90;if($tailCase299.$===-1&&$tailCase299.d.$===-1){{var $l91=$tailCase299.d;let $tailNext0=$l91;$tailState0=$tailNext0;continue $tailLoop;}}{return $l90;}};});
var $elm$core$Dict$moveRedRight=(function($l136){let $tailCase474=$l136;if($tailCase474.$===-1){if($tailCase474.d.$===-1){if($tailCase474.d.d.$===-1&&$tailCase474.d.d.a===0&&$tailCase474.e.$===-1){{var $l137=$tailCase474.a;var $l138=$tailCase474.b;var $l139=$tailCase474.c;var $l140=$tailCase474.d.a;var $l141=$tailCase474.d.b;var $l142=$tailCase474.d.c;var $l143=$tailCase474.d.d.b;var $l144=$tailCase474.d.d.c;var $l145=$tailCase474.d.d.d;var $l146=$tailCase474.d.d.e;var $l147=$tailCase474.d.e;var $l148=$tailCase474.e.a;var $l149=$tailCase474.e.b;var $l150=$tailCase474.e.c;var $l151=$tailCase474.e.d;var $l152=$tailCase474.e.e;return A5($elm$core$Dict$RBNode_elm_builtin,0,$l141,$l142,A5($elm$core$Dict$RBNode_elm_builtin,1,$l143,$l144,$l145,$l146),A5($elm$core$Dict$RBNode_elm_builtin,1,$l138,$l139,$l147,A5($elm$core$Dict$RBNode_elm_builtin,0,$l149,$l150,$l151,$l152)));}}if($tailCase474.e.$===-1){{var $l153=$tailCase474.a;var $l154=$tailCase474.b;var $l155=$tailCase474.c;var $l156=$tailCase474.d.a;var $l157=$tailCase474.d.b;var $l158=$tailCase474.d.c;var $l159=$tailCase474.d.d;var $l160=$tailCase474.d.e;var $l161=$tailCase474.e.a;var $l162=$tailCase474.e.b;var $l163=$tailCase474.e.c;var $l164=$tailCase474.e.d;var $l165=$tailCase474.e.e;let $tailCase472=$l153;if($tailCase472===1){{return A5($elm$core$Dict$RBNode_elm_builtin,1,$l154,$l155,A5($elm$core$Dict$RBNode_elm_builtin,0,$l157,$l158,$l159,$l160),A5($elm$core$Dict$RBNode_elm_builtin,0,$l162,$l163,$l164,$l165));}}{return A5($elm$core$Dict$RBNode_elm_builtin,1,$l154,$l155,A5($elm$core$Dict$RBNode_elm_builtin,0,$l157,$l158,$l159,$l160),A5($elm$core$Dict$RBNode_elm_builtin,0,$l162,$l163,$l164,$l165));}}}}}{return $l136;};});
var $elm$core$Dict$removeHelpPrepEQGT=F7(function($l70,$l71,$l72,$l73,$l74,$l75,$l76){let $tailCase262=$l75;if($tailCase262.$===-1&&$tailCase262.a===0){{var $l77=$tailCase262.b;var $l78=$tailCase262.c;var $l79=$tailCase262.d;var $l80=$tailCase262.e;return A5($elm$core$Dict$RBNode_elm_builtin,$l72,$l77,$l78,$l79,A5($elm$core$Dict$RBNode_elm_builtin,0,$l73,$l74,$l80,$l76));}}{let $tailCase261=$l76;if($tailCase261.$===-1){if($tailCase261.a===1){if($tailCase261.d.$===-1&&$tailCase261.d.a===1){{return ($elm$core$Dict$moveRedRight)($l71);}}if($tailCase261.d.$===-2){{return ($elm$core$Dict$moveRedRight)($l71);}}}}{return $l71;}};});
var $elm$core$Dict$removeHelp=F2(function($l57,$l58){let $tailCase238=$l58;if($tailCase238.$===-2){{return $elm$core$Dict$RBEmpty_elm_builtin;}}{var $l59=$tailCase238.a;var $l60=$tailCase238.b;var $l61=$tailCase238.c;var $l62=$tailCase238.d;var $l63=$tailCase238.e;if((_Utils_cmp($l57,$l60)<0)){let $tailCase224=$l62;if($tailCase224.$===-1&&$tailCase224.a===1){{var $l64=$tailCase224.d;let $tailCase213=$l64;if($tailCase213.$===-1&&$tailCase213.a===0){{return A5($elm$core$Dict$RBNode_elm_builtin,$l59,$l60,$l61,A2($elm$core$Dict$removeHelp,$l57,$l62),$l63);}}{let $tailCase212=($elm$core$Dict$moveRedLeft)($l58);if($tailCase212.$===-1){{var $l65=$tailCase212.a;var $l66=$tailCase212.b;var $l67=$tailCase212.c;var $l68=$tailCase212.d;var $l69=$tailCase212.e;return A5($elm$core$Dict$balance,$l65,$l66,$l67,A2($elm$core$Dict$removeHelp,$l57,$l68),$l69);}}{return $elm$core$Dict$RBEmpty_elm_builtin;}}}}{return A5($elm$core$Dict$RBNode_elm_builtin,$l59,$l60,$l61,A2($elm$core$Dict$removeHelp,$l57,$l62),$l63);}}else{return A2($elm$core$Dict$removeHelpEQGT,$l57,A7($elm$core$Dict$removeHelpPrepEQGT,$l57,$l58,$l59,$l60,$l61,$l62,$l63));}};});
var $elm$core$Dict$removeHelpEQGT=F2(function($l81,$l82){let $tailCase293=$l82;if($tailCase293.$===-1){{var $l83=$tailCase293.a;var $l84=$tailCase293.b;var $l85=$tailCase293.c;var $l86=$tailCase293.d;var $l87=$tailCase293.e;if(_Utils_eq($l81,$l84)){let $tailCase280=($elm$core$Dict$getMin)($l87);if($tailCase280.$===-1){{var $l88=$tailCase280.b;var $l89=$tailCase280.c;return A5($elm$core$Dict$balance,$l83,$l88,$l89,$l86,($elm$core$Dict$removeMin)($l87));}}{return $elm$core$Dict$RBEmpty_elm_builtin;}}else{return A5($elm$core$Dict$balance,$l83,$l84,$l85,$l86,A2($elm$core$Dict$removeHelp,$l81,$l87));}}}{return $elm$core$Dict$RBEmpty_elm_builtin;};});
var $elm$core$Dict$remove=F2(function($l50,$l51){let $tailCase180=A2($elm$core$Dict$removeHelp,$l50,$l51);if($tailCase180.$===-1&&$tailCase180.a===0){{var $l52=$tailCase180.b;var $l53=$tailCase180.c;var $l54=$tailCase180.d;var $l55=$tailCase180.e;return A5($elm$core$Dict$RBNode_elm_builtin,1,$l52,$l53,$l54,$l55);}}{var $l56=$tailCase180;return $l56;};});
var $elm$core$Dict$update=F3(function($l166,$l167,$l168){let $tailCase490=($l167)(A2($elm$core$Dict$get,$l166,$l168));if($tailCase490.$===0){{var $l169=$tailCase490.a;return A3($elm$core$Dict$insert,$l166,$l169,$l168);}}{return A2($elm$core$Dict$remove,$l166,$l168);};});
var $elm$core$Dict$foldr=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l215=$tailState0;var $l216=$tailState1;var $l217=$tailState2;let $tailCase633=$l217;if($tailCase633.$===-2){{return $l216;}}{var $l218=$tailCase633.b;var $l219=$tailCase633.c;var $l220=$tailCase633.d;var $l221=$tailCase633.e;let $tailNext0=$l215;let $tailNext1=A3($l215,$l218,$l219,A3($elm$core$Dict$foldr,$l215,$l216,$l221));let $tailNext2=$l220;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}};});
var $elm$core$Dict$toList=(function($l242){return A3($elm$core$Dict$foldr,F3(function($arg699_0,$arg699_1,$arg699_2){var $l243=$arg699_0;var $l244=$arg699_1;var $l245=$arg699_2;return A2($elm$core$List$cons,_Utils_Tuple2($l243,$l244),$l245);}),_List_Nil,$l242);;});
var $elm$core$Dict$foldl=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l208=$tailState0;var $l209=$tailState1;var $l210=$tailState2;let $tailCase617=$l210;if($tailCase617.$===-2){{return $l209;}}{var $l211=$tailCase617.b;var $l212=$tailCase617.c;var $l213=$tailCase617.d;var $l214=$tailCase617.e;let $tailNext0=$l208;let $tailNext1=A3($l208,$l211,$l212,A3($elm$core$Dict$foldl,$l208,$l209,$l213));let $tailNext2=$l214;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}};});
var $elm$core$Dict$merge=F6(function($l182,$l183,$l184,$l185,$l186,$l187){{var $l188=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l191=$tailState0;var $l192=$tailState1;var $l193=$tailState2.a;var $l194=$tailState2.b;let $tailCase562=$l193;if(($tailCase562.$===0)){{return _Utils_Tuple2($l193,A3($l184,$l191,$l192,$l194));}}{var $l195=$tailCase562.a.a;var $l196=$tailCase562.a.b;var $l197=$tailCase562.b;if((_Utils_cmp($l195,$l191)<0)){let $tailNext0=$l191;let $tailNext1=$l192;let $tailNext2=_Utils_Tuple2($l197,A3($l182,$l195,$l196,$l194));$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}else{if((_Utils_cmp($l195,$l191)>0)){return _Utils_Tuple2($l193,A3($l184,$l191,$l192,$l194));}else{return _Utils_Tuple2($l197,A4($l183,$l195,$l196,$l192,$l194));}}}};});var $tailDestruct582_1=A3($elm$core$Dict$foldl,$l188,_Utils_Tuple2(($elm$core$Dict$toList)($l185),$l187),$l186);var $l189=$tailDestruct582_1.a;var $l190=$tailDestruct582_1.b;return A3($elm$core$List$foldl,F2(function($arg578_0,$arg578_1){var $l198=$arg578_0.a;var $l199=$arg578_0.b;var $l200=$arg578_1;return A3($l182,$l198,$l199,$l200);}),$l190,$l189);};});
var $elm$core$Dict$map=F2(function($l201,$l202){let $tailCase601=$l202;if($tailCase601.$===-2){{return $elm$core$Dict$RBEmpty_elm_builtin;}}{var $l203=$tailCase601.a;var $l204=$tailCase601.b;var $l205=$tailCase601.c;var $l206=$tailCase601.d;var $l207=$tailCase601.e;return A5($elm$core$Dict$RBNode_elm_builtin,$l203,$l204,A2($l201,$l204,$l205),A2($elm$core$Dict$map,$l201,$l206),A2($elm$core$Dict$map,$l201,$l207));};});
var $elm$core$Dict$keys=(function($l234){return A3($elm$core$Dict$foldr,F3(function($arg681_0,$arg681_1,$arg681_2){var $l235=$arg681_0;var $l236=$arg681_1;var $l237=$arg681_2;return A2($elm$core$List$cons,$l235,$l237);}),_List_Nil,$l234);;});
var $elm$core$Dict$values=(function($l238){return A3($elm$core$Dict$foldr,F3(function($arg689_0,$arg689_1,$arg689_2){var $l239=$arg689_0;var $l240=$arg689_1;var $l241=$arg689_2;return A2($elm$core$List$cons,$l240,$l241);}),_List_Nil,$l238);;});
var $elm$core$Dict$fromList=(function($l246){return A3($elm$core$List$foldl,F2(function($arg709_0,$arg709_1){var $l247=$arg709_0.a;var $l248=$arg709_0.b;var $l249=$arg709_1;return A3($elm$core$Dict$insert,$l247,$l248,$l249);}),$elm$core$Dict$empty,$l246);;});
var $elm$core$Platform$Cmd$batch=_Platform_batch;
var $elm$core$Platform$Cmd$none=($elm$core$Platform$Cmd$batch)(_List_Nil);
var $elm$core$Platform$Cmd$map=_Platform_map;
var $elm$core$Platform$Sub$batch=_Platform_batch;
var $elm$core$Platform$Sub$none=($elm$core$Platform$Sub$batch)(_List_Nil);
var $elm$core$Platform$Sub$map=_Platform_map;
var $elm$core$Platform$sendToApp=_Platform_sendToApp;
var $elm$core$Set$toList=(function($tailInput0){var $l15=$tailInput0;return ($elm$core$Dict$keys)($l15);;});
var $elm$core$Set$foldr=F3(function($l22,$l23,$tailInput2){var $l24=$tailInput2;return A3($elm$core$Dict$foldr,F3(function($arg72_0,$arg72_1,$arg72_2){var $l25=$arg72_0;var $l26=$arg72_2;return A2($l22,$l25,$l26);}),$l23,$l24);;});
var $elm$json$Json$Encode$encode=_Json_encode;
var $elm$json$Json$Encode$string=_Json_wrap;
var $elm$json$Json$Encode$list=F2(function($l0,$l1){return (_Json_wrap)(A3($elm$core$List$foldl,(_Json_addEntry)($l0),(_Json_emptyArray)(0),$l1));;});
var $elm$json$Json$Encode$object=(function($l6){return (_Json_wrap)(A3($elm$core$List$foldl,F2(function($arg46_0,$arg46_1){var $l7=$arg46_0.a;var $l8=$arg46_0.b;var $l9=$arg46_1;return A3(_Json_addField,$l7,$l8,$l9);}),(_Json_emptyObject)(0),$l6));;});
var $elm$json$Json$Decode$string=_Json_decodeString;
var $elm$json$Json$Decode$bool=_Json_decodeBool;
var $elm$json$Json$Decode$int=_Json_decodeInt;
var $elm$json$Json$Decode$null=_Json_decodeNull;
var $elm$json$Json$Decode$map=_Json_map1;
var $elm$json$Json$Decode$oneOf=_Json_oneOf;
var $elm$json$Json$Decode$nullable=(function($l0){return ($elm$json$Json$Decode$oneOf)(_List_fromArray([($elm$json$Json$Decode$null)($elm$core$Maybe$Nothing),A2($elm$json$Json$Decode$map,$elm$core$Maybe$Just,$l0)]));;});
var $elm$json$Json$Decode$list=_Json_decodeList;
var $elm$json$Json$Decode$keyValuePairs=_Json_decodeKeyValuePairs;
var $elm$json$Json$Decode$dict=(function($l1){return A2($elm$json$Json$Decode$map,$elm$core$Dict$fromList,($elm$json$Json$Decode$keyValuePairs)($l1));;});
var $elm$json$Json$Decode$field=_Json_decodeField;
var $elm$json$Json$Decode$at=F2(function($l8,$l9){return A3($elm$core$List$foldr,$elm$json$Json$Decode$field,$l9,$l8);;});
var $elm$json$Json$Decode$map2=_Json_map2;
var $elm$json$Json$Decode$map3=_Json_map3;
var $elm$json$Json$Decode$map6=_Json_map6;
var $elm$json$Json$Decode$map7=_Json_map7;
var $elm$json$Json$Decode$map8=_Json_map8;
var $elm$json$Json$Decode$decodeString=_Json_runOnString;
var $elm$json$Json$Decode$decodeValue=_Json_run;
var $elm$json$Json$Decode$Field=F2(function(c0,c1){return {$:0,a:c0,b:c1};});
var $elm$json$Json$Decode$Index=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$json$Json$Decode$OneOf=function(c0){return {$:2,a:c0};};
var $elm$json$Json$Decode$Failure=F2(function(c0,c1){return {$:3,a:c0,b:c1};});
var $elm$json$Json$Decode$indent=(function($l32){return A2($elm$core$String$join,'\n    ',A2($elm$core$String$split,'\n',$l32));;});
var $elm$json$Json$Decode$errorToString=(function($l11){return A2($elm$json$Json$Decode$errorToStringHelp,$l11,_List_Nil);;});
var $elm$json$Json$Decode$errorOneOf=F2(function($l30,$l31){return ('\n\n('+($elm$core$String$fromInt)(($l30 + 1))+') '+($elm$json$Json$Decode$indent)(($elm$json$Json$Decode$errorToString)($l31)));;});
var $elm$json$Json$Decode$errorToStringHelp=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l12=$tailState0;var $l13=$tailState1;let $tailCase190=$l12;if($tailCase190.$===0){{var $l14=$tailCase190.a;var $l15=$tailCase190.b;{var $l16=(function($case88){if($case88.$===1){{return false;}}{var $l18=$case88.a.a;var $l19=$case88.a.b;return (($elm$core$Char$isAlpha)($l18)&&A2($elm$core$String$all,$elm$core$Char$isAlphaNum,$l19));}})(($elm$core$String$uncons)($l14));var $l17=($l16?('.'+$l14):('[\''+$l14+'\']'));let $tailNext0=$l15;let $tailNext1=A2($elm$core$List$cons,$l17,$l13);$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}}if($tailCase190.$===1){{var $l20=$tailCase190.a;var $l21=$tailCase190.b;{var $l22=('['+($elm$core$String$fromInt)($l20)+']');let $tailNext0=$l21;let $tailNext1=A2($elm$core$List$cons,$l22,$l13);$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}}if($tailCase190.$===2){{var $l23=$tailCase190.a;let $tailCase166=$l23;if(($tailCase166.$===0)){{return ('Ran into a Json.Decode.oneOf with no possibilities'+(($l13.$===0)?'!':(' at json'+A2($elm$core$String$join,'',($elm$core$List$reverse)($l13)))));}}if(($tailCase166.$===1)&&($tailCase166.b.$===0)){{var $l24=$tailCase166.a;let $tailNext0=$l24;let $tailNext1=$l13;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}{{var $l25=(($l13.$===0)?'Json.Decode.oneOf':('The Json.Decode.oneOf at json'+A2($elm$core$String$join,'',($elm$core$List$reverse)($l13))));var $l26=($l25+' failed in the following '+($elm$core$String$fromInt)(($elm$core$List$length)($l23))+' ways:');return A2($elm$core$String$join,'\n\n',A2($elm$core$List$cons,$l26,A2($elm$core$List$indexedMap,$elm$json$Json$Decode$errorOneOf,$l23)));}}}}{var $l27=$tailCase190.a;var $l28=$tailCase190.b;{var $l29=(($l13.$===0)?'Problem with the given value:\n\n':('Problem with the value at json'+A2($elm$core$String$join,'',($elm$core$List$reverse)($l13))+':\n\n    '));return ($l29+($elm$json$Json$Decode$indent)(A2($elm$json$Json$Encode$encode,4,$l28))+'\n\n'+$l27);}}};});
var $elm$json$Json$Decode$succeed=_Json_succeed;
var $elm$json$Json$Decode$fail=_Json_fail;
var $elm$json$Json$Decode$andThen=_Json_andThen;
var $elm$json$Json$Decode$value=_Json_decodeValue;
var $elm$core$Task$succeed=_Scheduler_succeed;
var $elm$core$Task$andThen=_Scheduler_andThen;
var $elm$core$Task$map=F2(function($l0,$l1){return A2($elm$core$Task$andThen,(function($arg9_0){var $l2=$arg9_0;return ($elm$core$Task$succeed)(($l0)($l2));}),$l1);;});
var $elm$core$Task$map2=F3(function($l3,$l4,$l5){return A2($elm$core$Task$andThen,(function($arg25_0){var $l6=$arg25_0;return A2($elm$core$Task$andThen,(function($arg22_0){var $l7=$arg22_0;return ($elm$core$Task$succeed)(A2($l3,$l6,$l7));}),$l5);}),$l4);;});
var $elm$core$Task$sequence=(function($l35){return A3($elm$core$List$foldr,($elm$core$Task$map2)($elm$core$List$cons),($elm$core$Task$succeed)(_List_Nil),$l35);;});
var $elm$core$Task$Perform=function(value){return value;};
var $elm$core$Task$command=_Platform_leaf("Task");
var $elm$core$Task$perform=F2(function($l38,$l39){return ($elm$core$Task$command)(($elm$core$Task$Perform)(A2($elm$core$Task$map,$l38,$l39)));;});
var $elm$core$Task$cmdMap=F2(function($l42,$tailInput1){var $l43=$tailInput1;return ($elm$core$Task$Perform)(A2($elm$core$Task$map,$l42,$l43));;});
var $elm$core$Task$init=($elm$core$Task$succeed)(0);
var $elm$core$Task$spawnCmd=F2(function($l47,$tailInput1){var $l48=$tailInput1;return (_Scheduler_spawn)(A2($elm$core$Task$andThen,($elm$core$Platform$sendToApp)($l47),$l48));;});
var $elm$core$Task$onEffects=F3(function($l44,$l45,$l46){return A2($elm$core$Task$map,(function($arg167_0){return 0;}),($elm$core$Task$sequence)(A2($elm$core$List$map,($elm$core$Task$spawnCmd)($l44),$l45)));;});
var $elm$core$Task$onSelfMsg=F3(function($tailInput0,$tailInput1,$tailInput2){return ($elm$core$Task$succeed)(0);;});
var $elm$virtual_dom$VirtualDom$node=(function($l0){return (_VirtualDom_node)((_VirtualDom_noScript)($l0));;});
var $elm$virtual_dom$VirtualDom$text=_VirtualDom_text;
var $elm$virtual_dom$VirtualDom$map=_VirtualDom_map;
var $elm$virtual_dom$VirtualDom$style=_VirtualDom_style;
var $elm$virtual_dom$VirtualDom$on=_VirtualDom_on;
var $elm$virtual_dom$VirtualDom$Normal=function(c0){return {$:0,a:c0};};
var $elm$virtual_dom$VirtualDom$lazy2=_VirtualDom_lazy2;
var $elm$virtual_dom$VirtualDom$lazy3=_VirtualDom_lazy3;
var $elm$virtual_dom$VirtualDom$toHandlerInt=(function($l12){let $tailCase64=$l12;if($tailCase64.$===0){{return 0;}}if($tailCase64.$===1){{return 1;}}if($tailCase64.$===2){{return 2;}}{return 3;};});
var $elm$html$Html$node=$elm$virtual_dom$VirtualDom$node;
var $elm$html$Html$text=$elm$virtual_dom$VirtualDom$text;
var $elm$html$Html$map=$elm$virtual_dom$VirtualDom$map;
var $elm$html$Html$section=(_VirtualDom_node)('section');
var $elm$html$Html$header=(_VirtualDom_node)('header');
var $elm$html$Html$p=(_VirtualDom_node)('p');
var $elm$html$Html$ul=(_VirtualDom_node)('ul');
var $elm$html$Html$li=(_VirtualDom_node)('li');
var $elm$html$Html$div=(_VirtualDom_node)('div');
var $elm$html$Html$a=(_VirtualDom_node)('a');
var $elm$html$Html$code=(_VirtualDom_node)('code');
var $elm$html$Html$span=(_VirtualDom_node)('span');
var $elm$html$Html$button=(_VirtualDom_node)('button');
var $elm$html$Html$Attributes$style=$elm$virtual_dom$VirtualDom$style;
var $elm$html$Html$Attributes$stringProperty=F2(function($l1,$l2){return A2(_VirtualDom_property,$l1,($elm$json$Json$Encode$string)($l2));;});
var $elm$html$Html$Attributes$class=($elm$html$Html$Attributes$stringProperty)('className');
var $elm$html$Html$Attributes$id=($elm$html$Html$Attributes$stringProperty)('id');
var $elm$html$Html$Attributes$title=($elm$html$Html$Attributes$stringProperty)('title');
var $elm$html$Html$Attributes$href=(function($l18){return A2($elm$html$Html$Attributes$stringProperty,'href',(_VirtualDom_noJavaScriptUri)($l18));;});
var $elm$html$Html$Events$on=F2(function($l15,$l16){return A2($elm$virtual_dom$VirtualDom$on,$l15,($elm$virtual_dom$VirtualDom$Normal)($l16));;});
var $elm$html$Html$Events$onClick=(function($l0){return A2($elm$html$Html$Events$on,'click',($elm$json$Json$Decode$succeed)($l0));;});
var $elm$browser$Debugger$Expando$S=function(c0){return {$:0,a:c0};};
var $elm$browser$Debugger$Expando$Primitive=function(c0){return {$:1,a:c0};};
var $elm$browser$Debugger$Expando$Sequence=F3(function(c0,c1,c2){return {$:2,a:c0,b:c1,c:c2};});
var $elm$browser$Debugger$Expando$Dictionary=F2(function(c0,c1){return {$:3,a:c0,b:c1};});
var $elm$browser$Debugger$Expando$Record=F2(function(c0,c1){return {$:4,a:c0,b:c1};});
var $elm$browser$Debugger$Expando$Constructor=F3(function(c0,c1,c2){return {$:5,a:c0,b:c1,c:c2};});
var $elm$browser$Debugger$Expando$ListSeq=0;
var $elm$browser$Debugger$Expando$SetSeq=1;
var $elm$browser$Debugger$Expando$ArraySeq=2;
var $elm$browser$Debugger$Expando$seqTypeToString=F2(function($l0,$l1){let $tailCase19=$l1;switch($tailCase19){case 0:{return ('List('+($elm$core$String$fromInt)($l0)+')');}case 1:{return ('Set('+($elm$core$String$fromInt)($l0)+')');}default:{return ('Array('+($elm$core$String$fromInt)($l0)+')');}};});
var $elm$browser$Debugger$Expando$initHelp=F2(function($l3,$l4){let $tailCase127=$l4;if($tailCase127.$===0){{return $l4;}}if($tailCase127.$===1){{return $l4;}}if($tailCase127.$===2){{var $l5=$tailCase127.a;var $l6=$tailCase127.b;var $l7=$tailCase127.c;if($l3){return A3($elm$browser$Debugger$Expando$Sequence,$l5,false,A2($elm$core$List$map,($elm$browser$Debugger$Expando$initHelp)(false),$l7));}else{if((_Utils_cmp(($elm$core$List$length)($l7),8)<1)){return A3($elm$browser$Debugger$Expando$Sequence,$l5,false,$l7);}else{return $l4;}}}}if($tailCase127.$===3){{var $l8=$tailCase127.a;var $l9=$tailCase127.b;if($l3){return A2($elm$browser$Debugger$Expando$Dictionary,false,A2($elm$core$List$map,(function($arg63_0){var $l10=$arg63_0.a;var $l11=$arg63_0.b;return _Utils_Tuple2($l10,A2($elm$browser$Debugger$Expando$initHelp,false,$l11));}),$l9));}else{if((_Utils_cmp(($elm$core$List$length)($l9),8)<1)){return A2($elm$browser$Debugger$Expando$Dictionary,false,$l9);}else{return $l4;}}}}if($tailCase127.$===4){{var $l12=$tailCase127.a;var $l13=$tailCase127.b;if($l3){return A2($elm$browser$Debugger$Expando$Record,false,A2($elm$core$Dict$map,F2(function($arg87_0,$arg87_1){var $l14=$arg87_1;return A2($elm$browser$Debugger$Expando$initHelp,false,$l14);}),$l13));}else{if((_Utils_cmp(($elm$core$Dict$size)($l13),4)<1)){return A2($elm$browser$Debugger$Expando$Record,false,$l13);}else{return $l4;}}}}{var $l15=$tailCase127.a;var $l16=$tailCase127.b;var $l17=$tailCase127.c;if($l3){return A3($elm$browser$Debugger$Expando$Constructor,$l15,false,A2($elm$core$List$map,($elm$browser$Debugger$Expando$initHelp)(false),$l17));}else{if((_Utils_cmp(($elm$core$List$length)($l17),4)<1)){return A3($elm$browser$Debugger$Expando$Constructor,$l15,false,$l17);}else{return $l4;}}};});
var $elm$browser$Debugger$Expando$init=(function($l2){return A2($elm$browser$Debugger$Expando$initHelp,true,(_Debugger_init)($l2));;});
var $elm$browser$Debugger$Expando$mergeHelp=F2(function($l20,$l21){let $tailCase170=_Utils_Tuple2($l20,$l21);if($tailCase170.b.$===0){{return $l21;}}if($tailCase170.b.$===1){{return $l21;}}if($tailCase170.a.$===2&&$tailCase170.b.$===2){{var $l22=$tailCase170.a.b;var $l23=$tailCase170.a.c;var $l24=$tailCase170.b.a;var $l25=$tailCase170.b.c;return A3($elm$browser$Debugger$Expando$Sequence,$l24,$l22,A2($elm$browser$Debugger$Expando$mergeListHelp,$l23,$l25));}}if($tailCase170.a.$===3&&$tailCase170.b.$===3){{var $l26=$tailCase170.a.a;var $l27=$tailCase170.b.b;return A2($elm$browser$Debugger$Expando$Dictionary,$l26,$l27);}}if($tailCase170.a.$===4&&$tailCase170.b.$===4){{var $l28=$tailCase170.a.a;var $l29=$tailCase170.a.b;var $l30=$tailCase170.b.b;return A2($elm$browser$Debugger$Expando$Record,$l28,A2($elm$core$Dict$map,($elm$browser$Debugger$Expando$mergeDictHelp)($l29),$l30));}}if($tailCase170.a.$===5&&$tailCase170.b.$===5){{var $l31=$tailCase170.a.b;var $l32=$tailCase170.a.c;var $l33=$tailCase170.b.a;var $l34=$tailCase170.b.c;return A3($elm$browser$Debugger$Expando$Constructor,$l33,$l31,A2($elm$browser$Debugger$Expando$mergeListHelp,$l32,$l34));}}{return $l21;};});
var $elm$browser$Debugger$Expando$mergeDictHelp=F3(function($l41,$l42,$l43){let $tailCase195=A2($elm$core$Dict$get,$l42,$l41);if($tailCase195.$===1){{return $l43;}}{var $l44=$tailCase195.a;return A2($elm$browser$Debugger$Expando$mergeHelp,$l44,$l43);};});
var $elm$browser$Debugger$Expando$mergeListHelp=F2(function($l35,$l36){let $tailCase185=_Utils_Tuple2($l35,$l36);if(($tailCase185.a.$===0)){{return $l36;}}if(($tailCase185.b.$===0)){{return $l36;}}{var $l37=$tailCase185.a.a;var $l38=$tailCase185.a.b;var $l39=$tailCase185.b.a;var $l40=$tailCase185.b.b;return A2($elm$core$List$cons,A2($elm$browser$Debugger$Expando$mergeHelp,$l37,$l39),A2($elm$browser$Debugger$Expando$mergeListHelp,$l38,$l40));};});
var $elm$browser$Debugger$Expando$merge=F2(function($l18,$l19){return A2($elm$browser$Debugger$Expando$mergeHelp,$l19,(_Debugger_init)($l18));;});
var $elm$browser$Debugger$Expando$Toggle=({$:0});
var $elm$browser$Debugger$Expando$Index=F3(function(c0,c1,c2){return {$:1,a:c0,b:c1,c:c2};});
var $elm$browser$Debugger$Expando$Field=F2(function(c0,c1){return {$:2,a:c0,b:c1};});
var $elm$browser$Debugger$Expando$updateIndex=F3(function($l70,$l71,$l72){let $tailCase327=$l72;if(($tailCase327.$===0)){{return _List_Nil;}}{var $l73=$tailCase327.a;var $l74=$tailCase327.b;if((_Utils_cmp($l70,0)<1)){return A2($elm$core$List$cons,($l71)($l73),$l74);}else{return A2($elm$core$List$cons,$l73,A3($elm$browser$Debugger$Expando$updateIndex,($l70 - 1),$l71,$l74));}};});
var $elm$browser$Debugger$Expando$update=F2(function($l45,$l46){let $tailCase306=$l46;if($tailCase306.$===0){{return $l46;}}if($tailCase306.$===1){{return $l46;}}if($tailCase306.$===2){{var $l47=$tailCase306.a;var $l48=$tailCase306.b;var $l49=$tailCase306.c;let $tailCase221=$l45;if($tailCase221.$===0){{return A3($elm$browser$Debugger$Expando$Sequence,$l47,(!$l48),$l49);}}if($tailCase221.$===1){if($tailCase221.a===0){{var $l50=$tailCase221.b;var $l51=$tailCase221.c;return A3($elm$browser$Debugger$Expando$Sequence,$l47,$l48,A3($elm$browser$Debugger$Expando$updateIndex,$l50,($elm$browser$Debugger$Expando$update)($l51),$l49));}}{return $l46;}}{return $l46;}}}if($tailCase306.$===3){{var $l52=$tailCase306.a;var $l53=$tailCase306.b;let $tailCase263=$l45;if($tailCase263.$===0){{return A2($elm$browser$Debugger$Expando$Dictionary,(!$l52),$l53);}}if($tailCase263.$===1){{var $l54=$tailCase263.a;var $l55=$tailCase263.b;var $l56=$tailCase263.c;let $tailCase261=$l54;switch($tailCase261){case 0:{return $l46;}case 1:{return A2($elm$browser$Debugger$Expando$Dictionary,$l52,A3($elm$browser$Debugger$Expando$updateIndex,$l55,(function($arg242_0){var $l57=$arg242_0.a;var $l58=$arg242_0.b;return _Utils_Tuple2(A2($elm$browser$Debugger$Expando$update,$l56,$l57),$l58);}),$l53));}default:{return A2($elm$browser$Debugger$Expando$Dictionary,$l52,A3($elm$browser$Debugger$Expando$updateIndex,$l55,(function($arg257_0){var $l59=$arg257_0.a;var $l60=$arg257_0.b;return _Utils_Tuple2($l59,A2($elm$browser$Debugger$Expando$update,$l56,$l60));}),$l53));}}}}{return $l46;}}}if($tailCase306.$===4){{var $l61=$tailCase306.a;var $l62=$tailCase306.b;let $tailCase282=$l45;if($tailCase282.$===0){{return A2($elm$browser$Debugger$Expando$Record,(!$l61),$l62);}}if($tailCase282.$===1){{return $l46;}}{var $l63=$tailCase282.a;var $l64=$tailCase282.b;return A2($elm$browser$Debugger$Expando$Record,$l61,A3($elm$core$Dict$update,$l63,($elm$browser$Debugger$Expando$updateField)($l64),$l62));}}}{var $l65=$tailCase306.a;var $l66=$tailCase306.b;var $l67=$tailCase306.c;let $tailCase305=$l45;if($tailCase305.$===0){{return A3($elm$browser$Debugger$Expando$Constructor,$l65,(!$l66),$l67);}}if($tailCase305.$===1){if($tailCase305.a===0){{var $l68=$tailCase305.b;var $l69=$tailCase305.c;return A3($elm$browser$Debugger$Expando$Constructor,$l65,$l66,A3($elm$browser$Debugger$Expando$updateIndex,$l68,($elm$browser$Debugger$Expando$update)($l69),$l67));}}{return $l46;}}{return $l46;}};});
var $elm$browser$Debugger$Expando$updateField=F2(function($l75,$l76){let $tailCase336=$l76;if($tailCase336.$===1){{return $l76;}}{var $l77=$tailCase336.a;return ($elm$core$Maybe$Just)(A2($elm$browser$Debugger$Expando$update,$l75,$l77));};});
var $elm$browser$Debugger$Expando$blue=A2($elm$html$Html$Attributes$style,'color','rgb(28, 0, 207)');
var $elm$browser$Debugger$Expando$red=A2($elm$html$Html$Attributes$style,'color','rgb(196, 26, 22)');
var $elm$browser$Debugger$Expando$leftPad=(function($l186){let $tailCase1134=$l186;if($tailCase1134.$===1){{return _List_Nil;}}{return _List_fromArray([A2($elm$html$Html$Attributes$style,'padding-left','4ch')]);};});
var $elm$browser$Debugger$Expando$purple=A2($elm$html$Html$Attributes$style,'color','rgb(136, 19, 145)');
var $elm$browser$Debugger$Expando$makeArrow=(function($l185){return A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'color','#777'),A2($elm$html$Html$Attributes$style,'padding-left','2ch'),A2($elm$html$Html$Attributes$style,'width','2ch'),A2($elm$html$Html$Attributes$style,'display','inline-block')]),_List_fromArray([($elm$html$Html$text)($l185)]));;});
var $elm$browser$Debugger$Expando$lineStarter=F3(function($l180,$l181,$l182){{var $l183=(function($case1083){if($case1083.$===1){{return ($elm$browser$Debugger$Expando$makeArrow)('');}}if($case1083.$===0&&$case1083.a===true){{return ($elm$browser$Debugger$Expando$makeArrow)('▸');}}{return ($elm$browser$Debugger$Expando$makeArrow)('▾');}})($l181);let $tailCase1102=$l180;if($tailCase1102.$===1){{return A2($elm$core$List$cons,$l183,$l182);}}{var $l184=$tailCase1102.a;return A2($elm$core$List$cons,$l183,A2($elm$core$List$cons,A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$purple]),_List_fromArray([($elm$html$Html$text)($l184)])),A2($elm$core$List$cons,($elm$html$Html$text)(' = '),$l182)));}};});
var $elm$browser$Debugger$Expando$viewExtraTinyRecord=F3(function($l172,$l173,$l174){let $tailCase1072=$l174;if(($tailCase1072.$===0)){{return _Utils_Tuple2(($l172 + 1),_List_fromArray([($elm$html$Html$text)('}')]));}}{var $l175=$tailCase1072.a;var $l176=$tailCase1072.b;{var $l177=(($l172 + ($elm$core$String$length)($l175)) + 1);if((_Utils_cmp($l177,18)>0)){return _Utils_Tuple2(($l172 + 2),_List_fromArray([($elm$html$Html$text)('…}')]));}else{{var $tailDestruct1069_0=A3($elm$browser$Debugger$Expando$viewExtraTinyRecord,$l177,',',$l176);var $l178=$tailDestruct1069_0.a;var $l179=$tailDestruct1069_0.b;return _Utils_Tuple2($l178,A2($elm$core$List$cons,($elm$html$Html$text)($l173),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$purple]),_List_fromArray([($elm$html$Html$text)($l175)])),$l179)));}}}};});
var $elm$browser$Debugger$Expando$elideMiddle=(function($l156){if((_Utils_cmp(($elm$core$String$length)($l156),18)<1)){return $l156;}else{return (A2($elm$core$String$left,8,$l156)+'...'+A2($elm$core$String$right,8,$l156));};});
var $elm$browser$Debugger$Expando$viewTinyHelp=(function($l155){return _Utils_Tuple2(($elm$core$String$length)($l155),_List_fromArray([($elm$html$Html$text)($l155)]));;});
var $elm$browser$Debugger$Expando$viewExtraTiny=(function($l170){let $tailCase1022=$l170;if($tailCase1022.$===4){{var $l171=$tailCase1022.b;return A3($elm$browser$Debugger$Expando$viewExtraTinyRecord,0,'{',($elm$core$Dict$keys)($l171));}}{return ($elm$browser$Debugger$Expando$viewTiny)($l170);};});
var $elm$browser$Debugger$Expando$viewTinyRecordHelp=F3(function($l158,$l159,$l160){let $tailCase1010=$l160;if(($tailCase1010.$===0)){{return _Utils_Tuple2(($l158 + 2),_List_fromArray([($elm$html$Html$text)(' }')]));}}{var $l161=$tailCase1010.a.a;var $l162=$tailCase1010.a.b;var $l163=$tailCase1010.b;{var $l164=($elm$core$String$length)($l161);var $tailDestruct1009_1=($elm$browser$Debugger$Expando$viewExtraTiny)($l162);var $l165=$tailDestruct1009_1.a;var $l166=$tailDestruct1009_1.b;var $l167=((($l158 + $l164) + $l165) + 5);if((_Utils_cmp($l167,60)>0)){return _Utils_Tuple2(($l158 + 4),_List_fromArray([($elm$html$Html$text)(', … }')]));}else{{var $tailDestruct1007_0=A3($elm$browser$Debugger$Expando$viewTinyRecordHelp,$l167,', ',$l163);var $l168=$tailDestruct1007_0.a;var $l169=$tailDestruct1007_0.b;return _Utils_Tuple2($l168,A2($elm$core$List$cons,($elm$html$Html$text)($l159),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$purple]),_List_fromArray([($elm$html$Html$text)($l161)])),A2($elm$core$List$cons,($elm$html$Html$text)(' = '),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_Nil,$l166),$l169)))));}}}};});
var $elm$browser$Debugger$Expando$viewTinyRecord=(function($l157){if(($elm$core$Dict$isEmpty)($l157)){return _Utils_Tuple2(2,_List_fromArray([($elm$html$Html$text)('{}')]));}else{return A3($elm$browser$Debugger$Expando$viewTinyRecordHelp,0,'{ ',($elm$core$Dict$toList)($l157));};});
var $elm$browser$Debugger$Expando$viewTiny=(function($l143){let $tailCase906=$l143;if($tailCase906.$===0){{var $l144=$tailCase906.a;{var $l145=($elm$browser$Debugger$Expando$elideMiddle)($l144);return _Utils_Tuple2(($elm$core$String$length)($l145),_List_fromArray([A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$red]),_List_fromArray([($elm$html$Html$text)($l145)]))]));}}}if($tailCase906.$===1){{var $l146=$tailCase906.a;return _Utils_Tuple2(($elm$core$String$length)($l146),_List_fromArray([A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$blue]),_List_fromArray([($elm$html$Html$text)($l146)]))]));}}if($tailCase906.$===2){{var $l147=$tailCase906.a;var $l148=$tailCase906.c;return ($elm$browser$Debugger$Expando$viewTinyHelp)(A2($elm$browser$Debugger$Expando$seqTypeToString,($elm$core$List$length)($l148),$l147));}}if($tailCase906.$===3){{var $l149=$tailCase906.b;return ($elm$browser$Debugger$Expando$viewTinyHelp)(('Dict('+($elm$core$String$fromInt)(($elm$core$List$length)($l149))+')'));}}if($tailCase906.$===4){{var $l150=$tailCase906.b;return ($elm$browser$Debugger$Expando$viewTinyRecord)($l150);}}if($tailCase906.$===5&&($tailCase906.c.$===0)){{var $l151=$tailCase906.a;return ($elm$browser$Debugger$Expando$viewTinyHelp)(A2($elm$core$Maybe$withDefault,'Unit',$l151));}}{var $l152=$tailCase906.a;var $l153=$tailCase906.c;return ($elm$browser$Debugger$Expando$viewTinyHelp)((function($case904){if($case904.$===1){{return ('Tuple('+($elm$core$String$fromInt)(($elm$core$List$length)($l153))+')');}}{var $l154=$case904.a;return ($l154+' …');}})($l152));};});
var $elm$browser$Debugger$Expando$view=F2(function($l78,$l79){let $tailCase396=$l79;if($tailCase396.$===0){{var $l80=$tailCase396.a;return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l78),A3($elm$browser$Debugger$Expando$lineStarter,$l78,$elm$core$Maybe$Nothing,_List_fromArray([A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$red]),_List_fromArray([($elm$html$Html$text)($l80)]))])));}}if($tailCase396.$===1){{var $l81=$tailCase396.a;return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l78),A3($elm$browser$Debugger$Expando$lineStarter,$l78,$elm$core$Maybe$Nothing,_List_fromArray([A2($elm$html$Html$span,_List_fromArray([$elm$browser$Debugger$Expando$blue]),_List_fromArray([($elm$html$Html$text)($l81)]))])));}}if($tailCase396.$===2){{var $l82=$tailCase396.a;var $l83=$tailCase396.b;var $l84=$tailCase396.c;return A4($elm$browser$Debugger$Expando$viewSequence,$l78,$l82,$l83,$l84);}}if($tailCase396.$===3){{var $l85=$tailCase396.a;var $l86=$tailCase396.b;return A3($elm$browser$Debugger$Expando$viewDictionary,$l78,$l85,$l86);}}if($tailCase396.$===4){{var $l87=$tailCase396.a;var $l88=$tailCase396.b;return A3($elm$browser$Debugger$Expando$viewRecord,$l78,$l87,$l88);}}{var $l89=$tailCase396.a;var $l90=$tailCase396.b;var $l91=$tailCase396.c;return A4($elm$browser$Debugger$Expando$viewConstructor,$l78,$l89,$l90,$l91);};});
var $elm$browser$Debugger$Expando$viewConstructorEntry=F2(function($l141,$l142){return A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,0,$l141),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)(($elm$core$String$fromInt)($l141)),$l142));;});
var $elm$browser$Debugger$Expando$viewConstructorOpen=(function($l140){return A2($elm$html$Html$div,_List_Nil,A2($elm$core$List$indexedMap,$elm$browser$Debugger$Expando$viewConstructorEntry,$l140));;});
var $elm$browser$Debugger$Expando$viewConstructor=F4(function($l117,$l118,$l119,$l120){{var $l121=A2($elm$core$List$map,A2($elm$core$Basics$composeL,$elm$core$Tuple$second,$elm$browser$Debugger$Expando$viewExtraTiny),$l120);var $l122=(function($case682){if($case682.a.$===1){if(($case682.b.$===0)){{return _List_fromArray([($elm$html$Html$text)('()')]);}}if(($case682.b.$===1)){{var $l125=$case682.b.a;var $l126=$case682.b.b;return A2($elm$core$List$cons,($elm$html$Html$text)('( '),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_Nil,$l125),A3($elm$core$List$foldr,F2(function($arg646_0,$arg646_1){var $l127=$arg646_0;var $l128=$arg646_1;return A2($elm$core$List$cons,($elm$html$Html$text)(', '),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_Nil,$l127),$l128));}),_List_fromArray([($elm$html$Html$text)(' )')]),$l126)));}}}if($case682.a.$===0&&($case682.b.$===0)){{var $l129=$case682.a.a;return _List_fromArray([($elm$html$Html$text)($l129)]);}}{var $l130=$case682.a.a;var $l131=$case682.b.a;var $l132=$case682.b.b;return A2($elm$core$List$cons,($elm$html$Html$text)(($l130+' ')),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_Nil,$l131),A3($elm$core$List$foldr,F2(function($arg677_0,$arg677_1){var $l133=$arg677_0;var $l134=$arg677_1;return A2($elm$core$List$cons,($elm$html$Html$text)(' '),A2($elm$core$List$cons,A2($elm$html$Html$span,_List_Nil,$l133),$l134));}),_List_Nil,$l132)));}})(_Utils_Tuple2($l118,$l121));var $tailDestruct812_2=(function($case793){if(($case793.$===0)){{return _Utils_Tuple2($elm$core$Maybe$Nothing,A2($elm$html$Html$div,_List_Nil,_List_Nil));}}if(($case793.$===1)&&($case793.b.$===0)){{var $l135=$case793.a;return (function($case779){if($case779.$===0){{return _Utils_Tuple2($elm$core$Maybe$Nothing,A2($elm$html$Html$div,_List_Nil,_List_Nil));}}if($case779.$===1){{return _Utils_Tuple2($elm$core$Maybe$Nothing,A2($elm$html$Html$div,_List_Nil,_List_Nil));}}if($case779.$===2){{var $l136=$case779.c;return _Utils_Tuple2(($elm$core$Maybe$Just)($l119),($l119?A2($elm$html$Html$div,_List_Nil,_List_Nil):A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,0,0),($elm$browser$Debugger$Expando$viewSequenceOpen)($l136))));}}if($case779.$===3){{var $l137=$case779.b;return _Utils_Tuple2(($elm$core$Maybe$Just)($l119),($l119?A2($elm$html$Html$div,_List_Nil,_List_Nil):A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,0,0),($elm$browser$Debugger$Expando$viewDictionaryOpen)($l137))));}}if($case779.$===4){{var $l138=$case779.b;return _Utils_Tuple2(($elm$core$Maybe$Just)($l119),($l119?A2($elm$html$Html$div,_List_Nil,_List_Nil):A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,0,0),($elm$browser$Debugger$Expando$viewRecordOpen)($l138))));}}{var $l139=$case779.c;return _Utils_Tuple2(($elm$core$Maybe$Just)($l119),($l119?A2($elm$html$Html$div,_List_Nil,_List_Nil):A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,0,0),($elm$browser$Debugger$Expando$viewConstructorOpen)($l139))));}})($l135);}}{return _Utils_Tuple2(($elm$core$Maybe$Just)($l119),($l119?A2($elm$html$Html$div,_List_Nil,_List_Nil):($elm$browser$Debugger$Expando$viewConstructorOpen)($l120)));}})($l120);var $l123=$tailDestruct812_2.a;var $l124=$tailDestruct812_2.b;return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l117),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Events$onClick)($elm$browser$Debugger$Expando$Toggle)]),A3($elm$browser$Debugger$Expando$lineStarter,$l117,$l123,$l122)),$l124]));};});
var $elm$browser$Debugger$Expando$viewSequenceOpen=(function($l97){return A2($elm$html$Html$div,_List_Nil,A2($elm$core$List$indexedMap,$elm$browser$Debugger$Expando$viewConstructorEntry,$l97));;});
var $elm$browser$Debugger$Expando$viewSequence=F4(function($l92,$l93,$l94,$l95){{var $l96=A2($elm$browser$Debugger$Expando$seqTypeToString,($elm$core$List$length)($l95),$l93);return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l92),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Events$onClick)($elm$browser$Debugger$Expando$Toggle)]),A3($elm$browser$Debugger$Expando$lineStarter,$l92,($elm$core$Maybe$Just)($l94),_List_fromArray([($elm$html$Html$text)($l96)]))),($l94?($elm$html$Html$text)(''):($elm$browser$Debugger$Expando$viewSequenceOpen)($l95))]));};});
var $elm$browser$Debugger$Expando$viewRecordEntry=(function($tailInput0){var $l115=$tailInput0.a;var $l116=$tailInput0.b;return A2($elm$html$Html$map,($elm$browser$Debugger$Expando$Field)($l115),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)($l115),$l116));;});
var $elm$browser$Debugger$Expando$viewRecordOpen=(function($l114){return A2($elm$html$Html$div,_List_Nil,A2($elm$core$List$map,$elm$browser$Debugger$Expando$viewRecordEntry,($elm$core$Dict$toList)($l114)));;});
var $elm$browser$Debugger$Expando$viewRecord=F3(function($l108,$l109,$l110){{var $tailDestruct595_0=($l109?_Utils_Tuple3(($elm$core$Tuple$second)(($elm$browser$Debugger$Expando$viewTinyRecord)($l110)),($elm$html$Html$text)(''),($elm$html$Html$text)('')):_Utils_Tuple3(_List_fromArray([($elm$html$Html$text)('{')]),($elm$browser$Debugger$Expando$viewRecordOpen)($l110),A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)(($elm$core$Maybe$Just)(0)),_List_fromArray([($elm$html$Html$text)('}')]))));var $l111=$tailDestruct595_0.a;var $l112=$tailDestruct595_0.b;var $l113=$tailDestruct595_0.c;return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l108),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Events$onClick)($elm$browser$Debugger$Expando$Toggle)]),A3($elm$browser$Debugger$Expando$lineStarter,$l108,($elm$core$Maybe$Just)($l109),$l111)),$l112,$l113]));};});
var $elm$browser$Debugger$Expando$viewDictionaryEntry=F2(function($l103,$tailInput1){var $l104=$tailInput1.a;var $l105=$tailInput1.b;let $tailCase540=$l104;if($tailCase540.$===0){{var $l106=$tailCase540.a;return A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,2,$l103),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)($l106),$l105));}}if($tailCase540.$===1){{var $l107=$tailCase540.a;return A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,2,$l103),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)($l107),$l105));}}{return A2($elm$html$Html$div,_List_Nil,_List_fromArray([A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,1,$l103),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)('key'),$l104)),A2($elm$html$Html$map,A2($elm$browser$Debugger$Expando$Index,2,$l103),A2($elm$browser$Debugger$Expando$view,($elm$core$Maybe$Just)('value'),$l105))]));};});
var $elm$browser$Debugger$Expando$viewDictionaryOpen=(function($l102){return A2($elm$html$Html$div,_List_Nil,A2($elm$core$List$indexedMap,$elm$browser$Debugger$Expando$viewDictionaryEntry,$l102));;});
var $elm$browser$Debugger$Expando$viewDictionary=F3(function($l98,$l99,$l100){{var $l101=('Dict('+($elm$core$String$fromInt)(($elm$core$List$length)($l100))+')');return A2($elm$html$Html$div,($elm$browser$Debugger$Expando$leftPad)($l98),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Events$onClick)($elm$browser$Debugger$Expando$Toggle)]),A3($elm$browser$Debugger$Expando$lineStarter,$l98,($elm$core$Maybe$Just)($l99),_List_fromArray([($elm$html$Html$text)($l101)]))),($l99?($elm$html$Html$text)(''):($elm$browser$Debugger$Expando$viewDictionaryOpen)($l100))]));};});
var $elm$browser$Debugger$Report$CorruptHistory=({$:0});
var $elm$browser$Debugger$Report$VersionChanged=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$browser$Debugger$Report$MessageChanged=F2(function(c0,c1){return {$:2,a:c0,b:c1};});
var $elm$browser$Debugger$Report$SomethingChanged=function(c0){return {$:3,a:c0};};
var $elm$browser$Debugger$Report$AliasChange=function(c0){return {$:0,a:c0};};
var $elm$browser$Debugger$Report$UnionChange=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$browser$Debugger$Report$TagChanges=(function($r0){return (function($r1){return (function($r2){return (function($r3){return ({"H":$r2,"ab":$r3,"K":$r1,"O":$r0});});});});});
var $elm$browser$Debugger$Report$emptyTagChanges=(function($l0){return A4($elm$browser$Debugger$Report$TagChanges,_List_Nil,_List_Nil,_List_Nil,$l0);;});
var $elm$browser$Debugger$Report$hasTagChanges=(function($l1){return _Utils_eq($l1,A4($elm$browser$Debugger$Report$TagChanges,_List_Nil,_List_Nil,_List_Nil,true));;});
var $elm$browser$Debugger$Report$some=(function($l13){return (!($elm$core$List$isEmpty)($l13));;});
var $elm$browser$Debugger$Report$evaluateChange=(function($l8){let $tailCase58=$l8;if($tailCase58.$===0){{return 0;}}{var $l9=$tailCase58.b["O"];var $l10=$tailCase58.b["K"];var $l11=$tailCase58.b["H"];var $l12=$tailCase58.b["ab"];if(((!$l12)||(($elm$browser$Debugger$Report$some)($l10)||($elm$browser$Debugger$Report$some)($l9)))){return 0;}else{if(($elm$browser$Debugger$Report$some)($l11)){return 1;}else{return 2;}}};});
var $elm$browser$Debugger$Report$worstCase=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l4=$tailState0;var $l5=$tailState1;let $tailCase37=$l5;if(($tailCase37.$===0)){{return $l4;}}if(($tailCase37.$===1)){if($tailCase37.a===0){{return 0;}}if($tailCase37.a===1){{var $l6=$tailCase37.b;let $tailNext0=1;let $tailNext1=$l6;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}}{var $l7=$tailCase37.b;let $tailNext0=$l4;let $tailNext1=$l7;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}};});
var $elm$browser$Debugger$Report$evaluate=(function($l2){let $tailCase25=$l2;if($tailCase25.$===0){{return 0;}}if($tailCase25.$===1){{return 0;}}if($tailCase25.$===2){{return 0;}}{var $l3=$tailCase25.a;return A2($elm$browser$Debugger$Report$worstCase,2,A2($elm$core$List$map,$elm$browser$Debugger$Report$evaluateChange,$l3));};});
var $elm$browser$Debugger$Metadata$Metadata=(function($r0){return (function($r1){return ({"U":$r1,"G":$r0});});});
var $elm$browser$Debugger$Metadata$Versions=(function($r0){return ({"q":$r0});});
var $elm$browser$Debugger$Metadata$Types=(function($r0){return (function($r1){return (function($r2){return ({"I":$r1,"b":$r0,"V":$r2});});});});
var $elm$browser$Debugger$Metadata$Alias=(function($r0){return (function($r1){return ({"t":$r0,"T":$r1});});});
var $elm$browser$Debugger$Metadata$Union=(function($r0){return (function($r1){return ({"t":$r0,"S":$r1});});});
var $elm$browser$Debugger$Metadata$problemTable=_List_fromArray([_Utils_Tuple2(0,'->'),_Utils_Tuple2(1,'Json.Decode.Decoder'),_Utils_Tuple2(2,'Task.Task'),_Utils_Tuple2(3,'Process.Id'),_Utils_Tuple2(4,'WebSocket.LowLevel.WebSocket'),_Utils_Tuple2(5,'Http.Request'),_Utils_Tuple2(6,'Platform.Program'),_Utils_Tuple2(7,'VirtualDom.Node'),_Utils_Tuple2(7,'VirtualDom.Attribute')]);
var $elm$browser$Debugger$Metadata$hasProblem=F2(function($l12,$tailInput1){var $l13=$tailInput1.a;var $l14=$tailInput1.b;if(A2($elm$core$String$contains,$l14,$l12)){return ($elm$core$Maybe$Just)($l13);}else{return $elm$core$Maybe$Nothing;};});
var $elm$browser$Debugger$Metadata$findProblems=(function($l11){return A2($elm$core$List$filterMap,($elm$browser$Debugger$Metadata$hasProblem)($l11),$elm$browser$Debugger$Metadata$problemTable);;});
var $elm$browser$Debugger$Metadata$ProblemType=(function($r0){return (function($r1){return ({"m":$r0,"D":$r1});});});
var $elm$browser$Debugger$Metadata$collectBadUnions=F3(function($l7,$tailInput1,$l9){var $l8=$tailInput1["S"];let $tailCase48=A2($elm$core$List$concatMap,$elm$browser$Debugger$Metadata$findProblems,($elm$core$List$concat)(($elm$core$Dict$values)($l8)));if(($tailCase48.$===0)){{return $l9;}}{var $l10=$tailCase48;return A2($elm$core$List$cons,A2($elm$browser$Debugger$Metadata$ProblemType,$l7,$l10),$l9);};});
var $elm$browser$Debugger$Metadata$collectBadAliases=F3(function($l3,$tailInput1,$l5){var $l4=$tailInput1["T"];let $tailCase32=($elm$browser$Debugger$Metadata$findProblems)($l4);if(($tailCase32.$===0)){{return $l5;}}{var $l6=$tailCase32;return A2($elm$core$List$cons,A2($elm$browser$Debugger$Metadata$ProblemType,$l3,$l6),$l5);};});
var $elm$browser$Debugger$Metadata$Error=(function($r0){return (function($r1){return ({"b":$r0,"D":$r1});});});
var $elm$browser$Debugger$Metadata$isPortable=(function($tailInput0){var $l0=$tailInput0["U"];{var $l1=A3($elm$core$Dict$foldl,$elm$browser$Debugger$Metadata$collectBadAliases,_List_Nil,($l0)["I"]);let $tailCase20=A3($elm$core$Dict$foldl,$elm$browser$Debugger$Metadata$collectBadUnions,$l1,($l0)["V"]);if(($tailCase20.$===0)){{return $elm$core$Maybe$Nothing;}}{var $l2=$tailCase20;return ($elm$core$Maybe$Just)(A2($elm$browser$Debugger$Metadata$Error,($l0)["b"],$l2));}};});
var $elm$browser$Debugger$Metadata$checkTag=F4(function($l35,$l36,$l37,$l38){if(_Utils_eq($l36,$l37)){return $l38;}else{return _Utils_update($l38,({"K":A2($elm$core$List$cons,$l35,($l38)["K"])}));};});
var $elm$browser$Debugger$Metadata$addTag=F3(function($l33,$tailInput1,$l34){return _Utils_update($l34,({"H":A2($elm$core$List$cons,$l33,($l34)["H"])}));;});
var $elm$browser$Debugger$Metadata$removeTag=F3(function($l31,$tailInput1,$l32){return _Utils_update($l32,({"O":A2($elm$core$List$cons,$l31,($l32)["O"])}));;});
var $elm$browser$Debugger$Metadata$checkUnion=F4(function($l26,$l27,$l28,$l29){{var $l30=A6($elm$core$Dict$merge,$elm$browser$Debugger$Metadata$removeTag,$elm$browser$Debugger$Metadata$checkTag,$elm$browser$Debugger$Metadata$addTag,($l27)["S"],($l28)["S"],($elm$browser$Debugger$Report$emptyTagChanges)(_Utils_eq(($l27)["t"],($l28)["t"])));if(($elm$browser$Debugger$Report$hasTagChanges)($l30)){return $l29;}else{return A2($elm$core$List$cons,A2($elm$browser$Debugger$Report$UnionChange,$l26,$l30),$l29);}};});
var $elm$browser$Debugger$Metadata$checkAlias=F4(function($l22,$l23,$l24,$l25){if((_Utils_eq(($l23)["T"],($l24)["T"])&&_Utils_eq(($l23)["t"],($l24)["t"]))){return $l25;}else{return A2($elm$core$List$cons,($elm$browser$Debugger$Report$AliasChange)($l22),$l25);};});
var $elm$browser$Debugger$Metadata$ignore=F3(function($l19,$l20,$l21){return $l21;;});
var $elm$browser$Debugger$Metadata$checkTypes=F2(function($l17,$l18){if((!_Utils_eq(($l17)["b"],($l18)["b"]))){return A2($elm$browser$Debugger$Report$MessageChanged,($l17)["b"],($l18)["b"]);}else{return ($elm$browser$Debugger$Report$SomethingChanged)(A6($elm$core$Dict$merge,$elm$browser$Debugger$Metadata$ignore,$elm$browser$Debugger$Metadata$checkUnion,$elm$browser$Debugger$Metadata$ignore,($l17)["V"],($l18)["V"],A6($elm$core$Dict$merge,$elm$browser$Debugger$Metadata$ignore,$elm$browser$Debugger$Metadata$checkAlias,$elm$browser$Debugger$Metadata$ignore,($l17)["I"],($l18)["I"],_List_Nil)));};});
var $elm$browser$Debugger$Metadata$check=F2(function($l15,$l16){if((!_Utils_eq((($l15)["G"])["q"],(($l16)["G"])["q"]))){return A2($elm$browser$Debugger$Report$VersionChanged,(($l15)["G"])["q"],(($l16)["G"])["q"]);}else{return A2($elm$browser$Debugger$Metadata$checkTypes,($l15)["U"],($l16)["U"]);};});
var $elm$browser$Debugger$Metadata$decodeAlias=A3($elm$json$Json$Decode$map2,$elm$browser$Debugger$Metadata$Alias,A2($elm$json$Json$Decode$field,'args',($elm$json$Json$Decode$list)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'type',$elm$json$Json$Decode$string));
var $elm$browser$Debugger$Metadata$decodeUnion=A3($elm$json$Json$Decode$map2,$elm$browser$Debugger$Metadata$Union,A2($elm$json$Json$Decode$field,'args',($elm$json$Json$Decode$list)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'tags',($elm$json$Json$Decode$dict)(($elm$json$Json$Decode$list)($elm$json$Json$Decode$string))));
var $elm$browser$Debugger$Metadata$decodeTypes=A4($elm$json$Json$Decode$map3,$elm$browser$Debugger$Metadata$Types,A2($elm$json$Json$Decode$field,'message',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'aliases',($elm$json$Json$Decode$dict)($elm$browser$Debugger$Metadata$decodeAlias)),A2($elm$json$Json$Decode$field,'unions',($elm$json$Json$Decode$dict)($elm$browser$Debugger$Metadata$decodeUnion)));
var $elm$browser$Debugger$Metadata$decodeVersions=A2($elm$json$Json$Decode$map,$elm$browser$Debugger$Metadata$Versions,A2($elm$json$Json$Decode$field,'elm',$elm$json$Json$Decode$string));
var $elm$browser$Debugger$Metadata$decoder=A3($elm$json$Json$Decode$map2,$elm$browser$Debugger$Metadata$Metadata,A2($elm$json$Json$Decode$field,'versions',$elm$browser$Debugger$Metadata$decodeVersions),A2($elm$json$Json$Decode$field,'types',$elm$browser$Debugger$Metadata$decodeTypes));
var $elm$browser$Debugger$Metadata$decode=(function($l39){let $tailCase233=A2($elm$json$Json$Decode$decodeValue,$elm$browser$Debugger$Metadata$decoder,$l39);if($tailCase233.$===1){{return ($elm$core$Result$Err)(A2($elm$browser$Debugger$Metadata$Error,'The compiler is generating bad metadata. This is a compiler bug!',_List_Nil));}}{var $l40=$tailCase233.a;let $tailCase232=($elm$browser$Debugger$Metadata$isPortable)($l40);if($tailCase232.$===1){{return ($elm$core$Result$Ok)($l40);}}{var $l41=$tailCase232.a;return ($elm$core$Result$Err)($l41);}};});
var $elm$browser$Debugger$Metadata$encodeDict=F2(function($l52,$l53){return ($elm$json$Json$Encode$object)(($elm$core$Dict$toList)(A2($elm$core$Dict$map,F2(function($arg378_0,$arg378_1){var $l54=$arg378_0;var $l55=$arg378_1;return ($l52)($l55);}),$l53)));;});
var $elm$browser$Debugger$Metadata$encodeUnion=(function($tailInput0){var $l50=$tailInput0["t"];var $l51=$tailInput0["S"];return ($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('args',A2($elm$json$Json$Encode$list,$elm$json$Json$Encode$string,$l50)),_Utils_Tuple2('tags',A2($elm$browser$Debugger$Metadata$encodeDict,($elm$json$Json$Encode$list)($elm$json$Json$Encode$string),$l51))]));;});
var $elm$browser$Debugger$Metadata$encodeAlias=(function($tailInput0){var $l48=$tailInput0["t"];var $l49=$tailInput0["T"];return ($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('args',A2($elm$json$Json$Encode$list,$elm$json$Json$Encode$string,$l48)),_Utils_Tuple2('type',($elm$json$Json$Encode$string)($l49))]));;});
var $elm$browser$Debugger$Metadata$encodeTypes=(function($tailInput0){var $l45=$tailInput0["b"];var $l46=$tailInput0["V"];var $l47=$tailInput0["I"];return ($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('message',($elm$json$Json$Encode$string)($l45)),_Utils_Tuple2('aliases',A2($elm$browser$Debugger$Metadata$encodeDict,$elm$browser$Debugger$Metadata$encodeAlias,$l47)),_Utils_Tuple2('unions',A2($elm$browser$Debugger$Metadata$encodeDict,$elm$browser$Debugger$Metadata$encodeUnion,$l46))]));;});
var $elm$browser$Debugger$Metadata$encodeVersions=(function($tailInput0){var $l44=$tailInput0["q"];return ($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('elm',($elm$json$Json$Encode$string)($l44))]));;});
var $elm$browser$Debugger$Metadata$encode=(function($tailInput0){var $l42=$tailInput0["G"];var $l43=$tailInput0["U"];return ($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('versions',($elm$browser$Debugger$Metadata$encodeVersions)($l42)),_Utils_Tuple2('types',($elm$browser$Debugger$Metadata$encodeTypes)($l43))]));;});
var $elm$html$Html$Lazy$lazy2=$elm$virtual_dom$VirtualDom$lazy2;
var $elm$html$Html$Lazy$lazy3=$elm$virtual_dom$VirtualDom$lazy3;
var $elm$browser$Debugger$History$maxSnapshotSize=64;
var $elm$browser$Debugger$History$History=(function($r0){return (function($r1){return (function($r2){return ({"u":$r2,"E":$r1,"F":$r0});});});});
var $elm$browser$Debugger$History$RecentHistory=(function($r0){return (function($r1){return (function($r2){return ({"s":$r1,"y":$r0,"u":$r2});});});});
var $elm$browser$Debugger$History$Snapshot=(function($r0){return (function($r1){return ({"s":$r1,"y":$r0});});});
var $elm$browser$Debugger$History$empty=(function($l0){return A3($elm$browser$Debugger$History$History,$elm$core$Array$empty,A3($elm$browser$Debugger$History$RecentHistory,$l0,_List_Nil,0),0);;});
var $elm$browser$Debugger$History$size=(function($l1){return ($l1)["u"];;});
var $elm$browser$Debugger$History$getInitialModel=(function($tailInput0){var $l2=$tailInput0["F"];var $l3=$tailInput0["E"];let $tailCase19=A2($elm$core$Array$get,0,$l2);if($tailCase19.$===0){{var $l4=$tailCase19.a["y"];return $l4;}}{return ($l3)["y"];};});
var $elm$browser$Debugger$History$addRecent=F3(function($l26,$l27,$tailInput2){var $l28=$tailInput2["y"];var $l29=$tailInput2["s"];var $l30=$tailInput2["u"];if(_Utils_eq($l30,$elm$browser$Debugger$History$maxSnapshotSize)){return _Utils_Tuple2(($elm$core$Maybe$Just)(A2($elm$browser$Debugger$History$Snapshot,$l28,($elm$core$Array$fromList)($l29))),A3($elm$browser$Debugger$History$RecentHistory,$l27,_List_fromArray([$l26]),1));}else{return _Utils_Tuple2($elm$core$Maybe$Nothing,A3($elm$browser$Debugger$History$RecentHistory,$l28,A2($elm$core$List$cons,$l26,$l29),($l30 + 1)));};});
var $elm$browser$Debugger$History$add=F3(function($l18,$l19,$tailInput2){var $l20=$tailInput2["F"];var $l21=$tailInput2["E"];var $l22=$tailInput2["u"];let $tailCase92=A3($elm$browser$Debugger$History$addRecent,$l18,$l19,$l21);if($tailCase92.a.$===0){{var $l23=$tailCase92.a.a;var $l24=$tailCase92.b;return A3($elm$browser$Debugger$History$History,A2($elm$core$Array$push,$l23,$l20),$l24,($l22 + 1));}}{var $l25=$tailCase92.b;return A3($elm$browser$Debugger$History$History,$l20,$l25,($l22 + 1));};});
var $elm$browser$Debugger$History$jsToElm=_Debugger_unsafeCoerce;
var $elm$browser$Debugger$History$decoder=F2(function($l5,$l6){{var $l7=F2(function($l9,$tailInput1){var $l10=$tailInput1.a;var $l11=$tailInput1.b;{var $l12=($elm$browser$Debugger$History$jsToElm)($l9);return _Utils_Tuple2(A2($l6,$l12,$l10),A3($elm$browser$Debugger$History$add,$l12,$l10,$l11));};});var $l8=(function($l13){return A3($elm$core$List$foldl,$l7,_Utils_Tuple2($l5,($elm$browser$Debugger$History$empty)($l5)),$l13);;});return A2($elm$json$Json$Decode$map,$l8,($elm$json$Json$Decode$list)($elm$json$Json$Decode$value));};});
var $elm$browser$Debugger$History$elmToJs=_Debugger_unsafeCoerce;
var $elm$browser$Debugger$History$encodeHelp=F2(function($l16,$l17){return A3($elm$core$Array$foldl,$elm$core$List$cons,$l17,($l16)["s"]);;});
var $elm$browser$Debugger$History$encode=(function($tailInput0){var $l14=$tailInput0["F"];var $l15=$tailInput0["E"];return A2($elm$json$Json$Encode$list,$elm$browser$Debugger$History$elmToJs,A3($elm$core$Array$foldr,$elm$browser$Debugger$History$encodeHelp,($elm$core$List$reverse)(($l15)["s"]),$l14));;});
var $elm$browser$Debugger$History$undone=(function($tailInput0){let $tailState0=$tailInput0;$tailLoop:while(true){var $l43=$tailState0;let $tailCase213=$l43;if($tailCase213.$===1){{var $l44=$tailCase213.a;var $l45=$tailCase213.b;return _Utils_Tuple2($l45,$l44);}}{let $tailNext0=$l43;$tailState0=$tailNext0;continue $tailLoop;}};});
var $elm$browser$Debugger$History$Done=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$browser$Debugger$History$Stepping=F2(function(c0,c1){return {$:0,a:c0,b:c1};});
var $elm$browser$Debugger$History$getHelp=F3(function($l38,$l39,$l40){let $tailCase205=$l40;if($tailCase205.$===1){{return $l40;}}{var $l41=$tailCase205.a;var $l42=$tailCase205.b;if(_Utils_eq($l41,0)){return A2($elm$browser$Debugger$History$Done,$l39,($elm$core$Tuple$first)(A2($l38,$l39,$l42)));}else{return A2($elm$browser$Debugger$History$Stepping,($l41 - 1),($elm$core$Tuple$first)(A2($l38,$l39,$l42)));}};});
var $elm$browser$Debugger$History$get=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l31=$tailState0;var $l32=$tailState1;var $l33=$tailState2;{var $l34=($l33)["E"];var $l35=(($l33)["u"] - ($l34)["u"]);if((_Utils_cmp($l32,$l35)>-1)){return ($elm$browser$Debugger$History$undone)(A3($elm$core$List$foldr,($elm$browser$Debugger$History$getHelp)($l31),A2($elm$browser$Debugger$History$Stepping,($l32 - $l35),($l34)["y"]),($l34)["s"]));}else{let $tailCase176=A2($elm$core$Array$get,(($l32)/($elm$browser$Debugger$History$maxSnapshotSize)|0),($l33)["F"]);if($tailCase176.$===1){{let $tailNext0=$l31;let $tailNext1=$l32;let $tailNext2=$l33;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}}{var $l36=$tailCase176.a["y"];var $l37=$tailCase176.a["s"];return ($elm$browser$Debugger$History$undone)(A3($elm$core$Array$foldr,($elm$browser$Debugger$History$getHelp)($l31),A2($elm$browser$Debugger$History$Stepping,A2($elm$core$Basics$remainderBy,$elm$browser$Debugger$History$maxSnapshotSize,$l32),$l36),$l37));}}}};});
var $elm$browser$Debugger$History$styles=A3($elm$html$Html$node,'style',_List_Nil,_List_fromArray([($elm$html$Html$text)('\n\n.elm-debugger-entry {\n  cursor: pointer;\n  width: 100%;\n}\n\n.elm-debugger-entry:hover {\n  background-color: rgb(41, 41, 41);\n}\n\n.elm-debugger-entry-selected, .elm-debugger-entry-selected:hover {\n  background-color: rgb(10, 10, 10);\n}\n\n.elm-debugger-entry-content {\n  width: calc(100% - 7ch);\n  padding-top: 4px;\n  padding-bottom: 4px;\n  padding-left: 1ch;\n  text-overflow: ellipsis;\n  white-space: nowrap;\n  overflow: hidden;\n  display: inline-block;\n}\n\n.elm-debugger-entry-index {\n  color: #666;\n  width: 5ch;\n  padding-top: 4px;\n  padding-bottom: 4px;\n  padding-right: 1ch;\n  text-align: right;\n  display: block;\n  float: right;\n}\n\n')]));
var $elm$browser$Debugger$History$viewMessage=F3(function($l71,$l72,$l73){{var $l74=(_Utils_eq($l71,$l72)?'elm-debugger-entry elm-debugger-entry-selected':'elm-debugger-entry');var $l75=(_Debugger_messageToString)($l73);return A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)($l74),($elm$html$Html$Events$onClick)($l72)]),_List_fromArray([A2($elm$html$Html$span,_List_fromArray([($elm$html$Html$Attributes$title)($l75),($elm$html$Html$Attributes$class)('elm-debugger-entry-content')]),_List_fromArray([($elm$html$Html$text)($l75)])),A2($elm$html$Html$span,_List_fromArray([($elm$html$Html$Attributes$class)('elm-debugger-entry-index')]),_List_fromArray([($elm$html$Html$text)(($elm$core$String$fromInt)($l72))]))]));};});
var $elm$browser$Debugger$History$consMsg=F3(function($l67,$l68,$tailInput2){var $l69=$tailInput2.a;var $l70=$tailInput2.b;return _Utils_Tuple2(($l69 - 1),A2($elm$core$List$cons,A4($elm$html$Html$Lazy$lazy3,$elm$browser$Debugger$History$viewMessage,$l67,$l69,$l68),$l70));;});
var $elm$browser$Debugger$History$viewSnapshot=F3(function($l64,$l65,$tailInput2){var $l66=$tailInput2["s"];return A2($elm$html$Html$div,_List_Nil,($elm$core$Tuple$second)(A3($elm$core$Array$foldl,($elm$browser$Debugger$History$consMsg)($l64),_Utils_Tuple2(($l65 - 1),_List_Nil),$l66)));;});
var $elm$browser$Debugger$History$consSnapshot=F3(function($l58,$l59,$tailInput2){var $l60=$tailInput2.a;var $l61=$tailInput2.b;{var $l62=($l60 - $elm$browser$Debugger$History$maxSnapshotSize);var $l63=(((_Utils_cmp($l62,$l58)<1)&&(_Utils_cmp($l58,$l60)<0))?$l58:(-1));return _Utils_Tuple2(($l60 - $elm$browser$Debugger$History$maxSnapshotSize),A2($elm$core$List$cons,A4($elm$html$Html$Lazy$lazy3,$elm$browser$Debugger$History$viewSnapshot,$l63,$l60,$l59),$l61));};});
var $elm$browser$Debugger$History$viewSnapshots=F2(function($l55,$l56){{var $l57=($elm$browser$Debugger$History$maxSnapshotSize * ($elm$core$Array$length)($l56));return A2($elm$html$Html$div,_List_Nil,($elm$core$Tuple$second)(A3($elm$core$Array$foldr,($elm$browser$Debugger$History$consSnapshot)($l55),_Utils_Tuple2($l57,_List_Nil),$l56)));};});
var $elm$browser$Debugger$History$view=F2(function($l46,$tailInput1){var $l47=$tailInput1["F"];var $l48=$tailInput1["E"];var $l49=$tailInput1["u"];{var $tailDestruct264_0=(function($case222){if($case222.$===1){{return _Utils_Tuple2((-1),'calc(100% - 24px)');}}{var $l54=$case222.a;return _Utils_Tuple2($l54,'calc(100% - 54px)');}})($l46);var $l50=$tailDestruct264_0.a;var $l51=$tailDestruct264_0.b;var $l52=A3($elm$html$Html$Lazy$lazy2,$elm$browser$Debugger$History$viewSnapshots,$l50,$l47);var $l53=($elm$core$Tuple$second)(A3($elm$core$List$foldl,($elm$browser$Debugger$History$consMsg)($l50),_Utils_Tuple2(($l49 - 1),_List_Nil),($l48)["s"]));return A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$id)('elm-debugger-sidebar'),A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'overflow-y','auto'),A2($elm$html$Html$Attributes$style,'height',$l51)]),A2($elm$core$List$cons,$elm$browser$Debugger$History$styles,A2($elm$core$List$cons,$l52,$l53)));};});
var $elm$browser$Debugger$Overlay$None=({$:0});
var $elm$browser$Debugger$Overlay$BadMetadata=function(c0){return {$:1,a:c0};};
var $elm$browser$Debugger$Overlay$BadImport=function(c0){return {$:2,a:c0};};
var $elm$browser$Debugger$Overlay$RiskyImport=F2(function(c0,c1){return {$:3,a:c0,b:c1};});
var $elm$browser$Debugger$Overlay$none=$elm$browser$Debugger$Overlay$None;
var $elm$browser$Debugger$Overlay$corruptImport=($elm$browser$Debugger$Overlay$BadImport)($elm$browser$Debugger$Report$CorruptHistory);
var $elm$browser$Debugger$Overlay$badMetadata=$elm$browser$Debugger$Overlay$BadMetadata;
var $elm$browser$Debugger$Overlay$close=F2(function($l0,$l1){let $tailCase15=$l1;if($tailCase15.$===0){{return $elm$core$Maybe$Nothing;}}if($tailCase15.$===1){{return $elm$core$Maybe$Nothing;}}if($tailCase15.$===2){{return $elm$core$Maybe$Nothing;}}{var $l2=$tailCase15.b;let $tailCase14=$l0;if($tailCase14===0){{return $elm$core$Maybe$Nothing;}}{return ($elm$core$Maybe$Just)($l2);}};});
var $elm$browser$Debugger$Overlay$uploadDecoder=A3($elm$json$Json$Decode$map2,F2(function($arg51_0,$arg51_1){var $l8=$arg51_0;var $l9=$arg51_1;return _Utils_Tuple2($l8,$l9);}),A2($elm$json$Json$Decode$field,'metadata',$elm$browser$Debugger$Metadata$decoder),A2($elm$json$Json$Decode$field,'history',$elm$json$Json$Decode$value));
var $elm$browser$Debugger$Overlay$assessImport=F2(function($l3,$l4){let $tailCase46=A2($elm$json$Json$Decode$decodeString,$elm$browser$Debugger$Overlay$uploadDecoder,$l4);if($tailCase46.$===1){{return ($elm$core$Result$Err)($elm$browser$Debugger$Overlay$corruptImport);}}{var $l5=$tailCase46.a.a;var $l6=$tailCase46.a.b;{var $l7=A2($elm$browser$Debugger$Metadata$check,$l5,$l3);let $tailCase44=($elm$browser$Debugger$Report$evaluate)($l7);switch($tailCase44){case 0:{return ($elm$core$Result$Err)(($elm$browser$Debugger$Overlay$BadImport)($l7));}case 1:{return ($elm$core$Result$Err)(A2($elm$browser$Debugger$Overlay$RiskyImport,$l7,$l6));}default:{return ($elm$core$Result$Ok)($l6);}}}};});
var $elm$browser$Debugger$Overlay$BlockNone=0;
var $elm$browser$Debugger$Overlay$BlockMost=1;
var $elm$browser$Debugger$Overlay$toBlockerType=F2(function($l10,$l11){let $tailCase69=$l11;if($tailCase69.$===0){{if($l10){return 2;}else{return 0;}}}if($tailCase69.$===1){{return 1;}}if($tailCase69.$===2){{return 1;}}{return 1;};});
var $elm$browser$Debugger$Overlay$button=F2(function($l70,$l71){return A2($elm$html$Html$span,_List_fromArray([($elm$html$Html$Events$onClick)($l70),A2($elm$html$Html$Attributes$style,'cursor','pointer')]),_List_fromArray([($elm$html$Html$text)($l71)]));;});
var $elm$browser$Debugger$Overlay$viewImportExport=F3(function($l67,$l68,$l69){return A2($elm$html$Html$div,$l67,_List_fromArray([A2($elm$browser$Debugger$Overlay$button,$l68,'Import'),($elm$html$Html$text)(' / '),A2($elm$browser$Debugger$Overlay$button,$l69,'Export')]));;});
var $elm$browser$Debugger$Overlay$viewMiniControls=F2(function($l65,$l66){return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'position','fixed'),A2($elm$html$Html$Attributes$style,'bottom','0'),A2($elm$html$Html$Attributes$style,'right','6px'),A2($elm$html$Html$Attributes$style,'border-radius','4px'),A2($elm$html$Html$Attributes$style,'background-color','rgb(61, 61, 61)'),A2($elm$html$Html$Attributes$style,'color','white'),A2($elm$html$Html$Attributes$style,'font-family','monospace'),A2($elm$html$Html$Attributes$style,'pointer-events','auto'),A2($elm$html$Html$Attributes$style,'z-index','2147483647')]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'padding','6px'),A2($elm$html$Html$Attributes$style,'cursor','pointer'),A2($elm$html$Html$Attributes$style,'text-align','center'),A2($elm$html$Html$Attributes$style,'min-width','24ch'),($elm$html$Html$Events$onClick)(($l65)["al"])]),_List_fromArray([($elm$html$Html$text)(('Explore History ('+($elm$core$String$fromInt)($l66)+')'))])),A3($elm$browser$Debugger$Overlay$viewImportExport,_List_fromArray([A2($elm$html$Html$Attributes$style,'padding','4px 0'),A2($elm$html$Html$Attributes$style,'font-size','0.8em'),A2($elm$html$Html$Attributes$style,'text-align','center'),A2($elm$html$Html$Attributes$style,'background-color','rgb(50, 50, 50)')]),($l65)["ai"],($l65)["ag"])]));;});
var $elm$browser$Debugger$Overlay$Choose=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$browser$Debugger$Overlay$Accept=function(c0){return {$:0,a:c0};};
var $elm$browser$Debugger$Overlay$addCommas=(function($l51){let $tailCase622=$l51;if(($tailCase622.$===0)){{return '';}}if(($tailCase622.$===1)){if(($tailCase622.b.$===0)){{var $l52=$tailCase622.a;return $l52;}}if(($tailCase622.b.$===1)&&($tailCase622.b.b.$===0)){{var $l53=$tailCase622.a;var $l54=$tailCase622.b.a;return ($l53+' and '+$l54);}}}{var $l55=$tailCase622.a;var $l56=$tailCase622.b;return A2($elm$core$String$join,', ',_Utils_ap($l56,_List_fromArray([(' and '+$l55)])));};});
var $elm$browser$Debugger$Overlay$problemToString=(function($l50){let $tailCase605=$l50;switch($tailCase605){case 0:{return 'functions';}case 1:{return 'JSON decoders';}case 2:{return 'tasks';}case 3:{return 'processes';}case 4:{return 'web sockets';}case 5:{return 'HTTP requests';}case 6:{return 'programs';}default:{return 'virtual DOM values';}};});
var $elm$browser$Debugger$Overlay$viewCode=(function($l31){return A2($elm$html$Html$code,_List_Nil,_List_fromArray([($elm$html$Html$text)($l31)]));;});
var $elm$browser$Debugger$Overlay$viewProblemType=(function($tailInput0){var $l48=$tailInput0["m"];var $l49=$tailInput0["D"];return A2($elm$html$Html$li,_List_Nil,_List_fromArray([($elm$browser$Debugger$Overlay$viewCode)($l48),($elm$html$Html$text)((' can contain '+($elm$browser$Debugger$Overlay$addCommas)(A2($elm$core$List$map,$elm$browser$Debugger$Overlay$problemToString,$l49))+'.'))]));;});
var $elm$browser$Debugger$Overlay$goodNews2='\nfunction can pattern match on that data and call whatever functions, JSON\ndecoders, etc. you need. This makes the code much more explicit and easy to\nfollow for other readers (or you in a few months!)\n';
var $elm$browser$Debugger$Overlay$goodNews1='\nThe good news is that having values like this in your message type is not\nso great in the long run. You are better off using simpler data, like\n';
var $elm$browser$Debugger$Overlay$viewBadMetadata=(function($tailInput0){var $l46=$tailInput0["b"];var $l47=$tailInput0["D"];return _List_fromArray([A2($elm$html$Html$p,_List_Nil,_List_fromArray([($elm$html$Html$text)('The '),($elm$browser$Debugger$Overlay$viewCode)($l46),($elm$html$Html$text)(' type of your program cannot be reliably serialized for history files.')])),A2($elm$html$Html$p,_List_Nil,_List_fromArray([($elm$html$Html$text)('Functions cannot be serialized, nor can values that contain functions. This is a problem in these places:')])),A2($elm$html$Html$ul,_List_Nil,A2($elm$core$List$map,$elm$browser$Debugger$Overlay$viewProblemType,$l47)),A2($elm$html$Html$p,_List_Nil,_List_fromArray([($elm$html$Html$text)($elm$browser$Debugger$Overlay$goodNews1),A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)('https://guide.elm-lang.org/types/union_types.html')]),_List_fromArray([($elm$html$Html$text)('union types')])),($elm$html$Html$text)(', in your messages. From there, your '),($elm$browser$Debugger$Overlay$viewCode)('update'),($elm$html$Html$text)($elm$browser$Debugger$Overlay$goodNews2)]))]);;});
var $elm$browser$Debugger$Overlay$viewMention=F2(function($l39,$l40){let $tailCase521=A2($elm$core$List$map,$elm$browser$Debugger$Overlay$viewCode,($elm$core$List$reverse)($l39));if(($tailCase521.$===0)){{return ($elm$html$Html$text)('');}}if(($tailCase521.$===1)){if(($tailCase521.b.$===0)){{var $l41=$tailCase521.a;return A2($elm$html$Html$li,_List_Nil,_List_fromArray([($elm$html$Html$text)($l40),$l41,($elm$html$Html$text)('.')]));}}if(($tailCase521.b.$===1)&&($tailCase521.b.b.$===0)){{var $l42=$tailCase521.a;var $l43=$tailCase521.b.a;return A2($elm$html$Html$li,_List_Nil,_List_fromArray([($elm$html$Html$text)($l40),$l43,($elm$html$Html$text)(' and '),$l42,($elm$html$Html$text)('.')]));}}}{var $l44=$tailCase521.a;var $l45=$tailCase521.b;return A2($elm$html$Html$li,_List_Nil,A2($elm$core$List$cons,($elm$html$Html$text)($l40),_Utils_ap(A2($elm$core$List$intersperse,($elm$html$Html$text)(', '),($elm$core$List$reverse)($l45)),_List_fromArray([($elm$html$Html$text)(', and '),$l44,($elm$html$Html$text)('.')]))));};});
var $elm$browser$Debugger$Overlay$viewChange=(function($l32){return A2($elm$html$Html$li,_List_fromArray([A2($elm$html$Html$Attributes$style,'margin','8px 0')]),(function($case461){if($case461.$===0){{var $l33=$case461.a;return _List_fromArray([A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','1.5em')]),_List_fromArray([($elm$browser$Debugger$Overlay$viewCode)($l33)]))]);}}{var $l34=$case461.a;var $l35=$case461.b["O"];var $l36=$case461.b["K"];var $l37=$case461.b["H"];var $l38=$case461.b["ab"];return _List_fromArray([A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','1.5em')]),_List_fromArray([($elm$browser$Debugger$Overlay$viewCode)($l34)])),A2($elm$html$Html$ul,_List_fromArray([A2($elm$html$Html$Attributes$style,'list-style-type','disc'),A2($elm$html$Html$Attributes$style,'padding-left','2em')]),_List_fromArray([A2($elm$browser$Debugger$Overlay$viewMention,$l35,'Removed '),A2($elm$browser$Debugger$Overlay$viewMention,$l36,'Changed '),A2($elm$browser$Debugger$Overlay$viewMention,$l37,'Added ')])),($l38?($elm$html$Html$text)(''):($elm$html$Html$text)('This may be due to the fact that the type variable names changed.'))]);}})($l32));;});
var $elm$browser$Debugger$Overlay$explanationRisky='\nThis history seems old. It will work with this program, but some\nmessages have been added since the history was created:\n';
var $elm$browser$Debugger$Overlay$explanationBad='\nThe messages in this history do not match the messages handled by your\nprogram. I noticed changes in the following types:\n';
var $elm$browser$Debugger$Overlay$viewReport=F2(function($l24,$l25){let $tailCase387=$l25;if($tailCase387.$===0){{return _List_fromArray([($elm$html$Html$text)('Looks like this history file is corrupt. I cannot understand it.')]);}}if($tailCase387.$===1){{var $l26=$tailCase387.a;var $l27=$tailCase387.b;return _List_fromArray([($elm$html$Html$text)(('This history was created with Elm '+$l26+', but you are using Elm '+$l27+' right now.'))]);}}if($tailCase387.$===2){{var $l28=$tailCase387.a;var $l29=$tailCase387.b;return _List_fromArray([($elm$html$Html$text)(('To import some other history, the overall message type must'+' be the same. The old history has ')),($elm$browser$Debugger$Overlay$viewCode)($l28),($elm$html$Html$text)(' messages, but the new program works with '),($elm$browser$Debugger$Overlay$viewCode)($l29),($elm$html$Html$text)(' messages.')]);}}{var $l30=$tailCase387.a;return _List_fromArray([A2($elm$html$Html$p,_List_Nil,_List_fromArray([($elm$html$Html$text)(($l24?$elm$browser$Debugger$Overlay$explanationBad:$elm$browser$Debugger$Overlay$explanationRisky))])),A2($elm$html$Html$ul,_List_fromArray([A2($elm$html$Html$Attributes$style,'list-style-type','none'),A2($elm$html$Html$Attributes$style,'padding-left','20px')]),A2($elm$core$List$map,$elm$browser$Debugger$Overlay$viewChange,$l30))]);};});
var $elm$browser$Debugger$Overlay$viewButtons=(function($l57){{var $l58=F2(function($l60,$l61){return A2($elm$html$Html$button,_List_fromArray([A2($elm$html$Html$Attributes$style,'margin-right','20px'),($elm$html$Html$Events$onClick)($l60)]),_List_fromArray([($elm$html$Html$text)($l61)]));;});var $l59=(function($case652){if($case652.$===0){{var $l62=$case652.a;return _List_fromArray([A2($l58,1,$l62)]);}}{var $l63=$case652.a;var $l64=$case652.b;return _List_fromArray([A2($l58,0,$l63),A2($l58,1,$l64)]);}})($l57);return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'height','60px'),A2($elm$html$Html$Attributes$style,'line-height','60px'),A2($elm$html$Html$Attributes$style,'text-align','right'),A2($elm$html$Html$Attributes$style,'background-color','rgb(50, 50, 50)')]),$l59);};});
var $elm$browser$Debugger$Overlay$viewMessage=F4(function($l20,$l21,$l22,$l23){return A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$id)('elm-debugger-overlay'),A2($elm$html$Html$Attributes$style,'position','fixed'),A2($elm$html$Html$Attributes$style,'top','0'),A2($elm$html$Html$Attributes$style,'left','0'),A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'color','white'),A2($elm$html$Html$Attributes$style,'pointer-events','none'),A2($elm$html$Html$Attributes$style,'font-family','\'Trebuchet MS\', \'Lucida Grande\', \'Bitstream Vera Sans\', \'Helvetica Neue\', sans-serif'),A2($elm$html$Html$Attributes$style,'z-index','2147483647')]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'position','absolute'),A2($elm$html$Html$Attributes$style,'width','600px'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'padding-left','calc(50% - 300px)'),A2($elm$html$Html$Attributes$style,'padding-right','calc(50% - 300px)'),A2($elm$html$Html$Attributes$style,'background-color','rgba(200, 200, 200, 0.7)'),A2($elm$html$Html$Attributes$style,'pointer-events','auto')]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','36px'),A2($elm$html$Html$Attributes$style,'height','80px'),A2($elm$html$Html$Attributes$style,'background-color','rgb(50, 50, 50)'),A2($elm$html$Html$Attributes$style,'padding-left','22px'),A2($elm$html$Html$Attributes$style,'vertical-align','middle'),A2($elm$html$Html$Attributes$style,'line-height','80px')]),_List_fromArray([($elm$html$Html$text)($l21)])),A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$id)('elm-debugger-details'),A2($elm$html$Html$Attributes$style,'padding',' 8px 20px'),A2($elm$html$Html$Attributes$style,'overflow-y','auto'),A2($elm$html$Html$Attributes$style,'max-height','calc(100% - 156px)'),A2($elm$html$Html$Attributes$style,'background-color','rgb(61, 61, 61)')]),$l22),A2($elm$html$Html$map,($l20)["at"],($elm$browser$Debugger$Overlay$viewButtons)($l23))]))]));;});
var $elm$browser$Debugger$Overlay$view=F5(function($l12,$l13,$l14,$l15,$l16){let $tailCase194=$l16;if($tailCase194.$===0){{if($l14){return ($elm$html$Html$text)('');}else{if($l13){return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'cursor','pointer'),A2($elm$html$Html$Attributes$style,'text-align','center'),A2($elm$html$Html$Attributes$style,'pointer-events','auto'),A2($elm$html$Html$Attributes$style,'background-color','rgba(200, 200, 200, 0.7)'),A2($elm$html$Html$Attributes$style,'color','white'),A2($elm$html$Html$Attributes$style,'font-family','\'Trebuchet MS\', \'Lucida Grande\', \'Bitstream Vera Sans\', \'Helvetica Neue\', sans-serif'),A2($elm$html$Html$Attributes$style,'z-index','2147483646'),($elm$html$Html$Events$onClick)(($l12)["am"])]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'position','absolute'),A2($elm$html$Html$Attributes$style,'top','calc(50% - 40px)'),A2($elm$html$Html$Attributes$style,'font-size','80px'),A2($elm$html$Html$Attributes$style,'line-height','80px'),A2($elm$html$Html$Attributes$style,'height','80px'),A2($elm$html$Html$Attributes$style,'width','100%')]),_List_fromArray([($elm$html$Html$text)('Click to Resume')])),A2($elm$browser$Debugger$Overlay$viewMiniControls,$l12,$l15)]));}else{return A2($elm$browser$Debugger$Overlay$viewMiniControls,$l12,$l15);}}}}if($tailCase194.$===1){{var $l17=$tailCase194.a;return A4($elm$browser$Debugger$Overlay$viewMessage,$l12,'Cannot use Import or Export',($elm$browser$Debugger$Overlay$viewBadMetadata)($l17),($elm$browser$Debugger$Overlay$Accept)('Ok'));}}if($tailCase194.$===2){{var $l18=$tailCase194.a;return A4($elm$browser$Debugger$Overlay$viewMessage,$l12,'Cannot Import History',A2($elm$browser$Debugger$Overlay$viewReport,true,$l18),($elm$browser$Debugger$Overlay$Accept)('Ok'));}}{var $l19=$tailCase194.a;return A4($elm$browser$Debugger$Overlay$viewMessage,$l12,'Warning',A2($elm$browser$Debugger$Overlay$viewReport,false,$l19),A2($elm$browser$Debugger$Overlay$Choose,'Cancel','Import Anyway'));};});
var $elm$browser$Debugger$Main$getCurrentModel=(function($l6){let $tailCase20=$l6;if($tailCase20.$===0){{var $l7=$tailCase20.a;return $l7;}}{var $l8=$tailCase20.b;return $l8;};});
var $elm$browser$Debugger$Main$getUserModel=(function($l0){return ($elm$browser$Debugger$Main$getCurrentModel)(($l0)["d"]);;});
var $elm$browser$Debugger$Main$UserMsg=function(c0){return {$:1,a:c0};};
var $elm$browser$Debugger$Main$getLatestModel=(function($l3){let $tailCase16=$l3;if($tailCase16.$===0){{var $l4=$tailCase16.a;return $l4;}}{var $l5=$tailCase16.c;return $l5;};});
var $elm$browser$Debugger$Main$wrapSubs=F2(function($l1,$l2){return A2($elm$core$Platform$Sub$map,$elm$browser$Debugger$Main$UserMsg,($l1)(($elm$browser$Debugger$Main$getLatestModel)(($l2)["d"])));;});
var $elm$browser$Debugger$Main$Running=function(c0){return {$:0,a:c0};};
var $elm$browser$Debugger$Main$Paused=F3(function(c0,c1,c2){return {$:1,a:c0,b:c1,c:c2};});
var $elm$browser$Debugger$Main$isPaused=(function($l9){let $tailCase24=$l9;if($tailCase24.$===0){{return false;}}{return true;};});
var $elm$browser$Debugger$Main$wrapInit=F4(function($l10,$l11,$l12,$l13){{var $tailDestruct48_0=($l12)($l13);var $l14=$tailDestruct48_0.a;var $l15=$tailDestruct48_0.b;return _Utils_Tuple2(({"n":($elm$browser$Debugger$Expando$init)($l14),"l":($elm$browser$Debugger$History$empty)($l14),"ak":($elm$browser$Debugger$Metadata$decode)($l10),"r":$elm$browser$Debugger$Overlay$none,"i":$l11,"d":($elm$browser$Debugger$Main$Running)($l14)}),A2($elm$core$Platform$Cmd$map,$elm$browser$Debugger$Main$UserMsg,$l15));};});
var $elm$browser$Debugger$Main$NoOp=({$:0});
var $elm$browser$Debugger$Main$ExpandoMsg=function(c0){return {$:2,a:c0};};
var $elm$browser$Debugger$Main$Resume=({$:3});
var $elm$browser$Debugger$Main$Jump=function(c0){return {$:4,a:c0};};
var $elm$browser$Debugger$Main$Open=({$:5});
var $elm$browser$Debugger$Main$Up=({$:6});
var $elm$browser$Debugger$Main$Down=({$:7});
var $elm$browser$Debugger$Main$Import=({$:8});
var $elm$browser$Debugger$Main$Export=({$:9});
var $elm$browser$Debugger$Main$Upload=function(c0){return {$:10,a:c0};};
var $elm$browser$Debugger$Main$OverlayMsg=function(c0){return {$:11,a:c0};};
var $elm$browser$Debugger$Main$loadNewHistory=F3(function($l52,$l53,$l54){{var $l55=($elm$browser$Debugger$History$getInitialModel)(($l54)["l"]);var $l56=F2(function($l58,$l59){return ($elm$core$Tuple$first)(A2($l53,$l58,$l59));;});var $l57=A2($elm$browser$Debugger$History$decoder,$l55,$l56);let $tailCase356=A2($elm$json$Json$Decode$decodeValue,$l57,$l52);if($tailCase356.$===1){{return _Utils_Tuple2(_Utils_update($l54,({"r":$elm$browser$Debugger$Overlay$corruptImport})),$elm$core$Platform$Cmd$none);}}{var $l60=$tailCase356.a.a;var $l61=$tailCase356.a.b;return _Utils_Tuple2(_Utils_update($l54,({"n":($elm$browser$Debugger$Expando$init)($l60),"l":$l61,"r":$elm$browser$Debugger$Overlay$none,"d":($elm$browser$Debugger$Main$Running)($l60)})),$elm$core$Platform$Cmd$none);}};});
var $elm$browser$Debugger$Main$withGoodMetadata=F2(function($l48,$l49){let $tailCase322=($l48)["ak"];if($tailCase322.$===0){{var $l50=$tailCase322.a;return ($l49)($l50);}}{var $l51=$tailCase322.a;return _Utils_Tuple2(_Utils_update($l48,({"r":($elm$browser$Debugger$Overlay$badMetadata)($l51)})),$elm$core$Platform$Cmd$none);};});
var $elm$browser$Debugger$Main$download=F2(function($l44,$l45){{var $l46=($elm$browser$Debugger$History$size)($l45);var $l47=($elm$json$Json$Encode$object)(_List_fromArray([_Utils_Tuple2('metadata',($elm$browser$Debugger$Metadata$encode)($l44)),_Utils_Tuple2('history',($elm$browser$Debugger$History$encode)($l45))]));return A2($elm$core$Task$perform,(function($arg304_0){return $elm$browser$Debugger$Main$NoOp;}),A2(_Debugger_download,$l46,$l47));};});
var $elm$browser$Debugger$Main$upload=A2($elm$core$Task$perform,$elm$browser$Debugger$Main$Upload,(_Debugger_upload)(0));
var $elm$browser$Debugger$Main$scroll=(function($l43){return A2($elm$core$Task$perform,($elm$core$Basics$always)($elm$browser$Debugger$Main$NoOp),(_Debugger_scroll)($l43));;});
var $elm$browser$Debugger$Main$wrapUpdate=F3(function($tailInput0,$tailInput1,$tailInput2){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;$tailLoop:while(true){var $l16=$tailState0;var $l17=$tailState1;var $l18=$tailState2;let $tailCase271=$l17;if($tailCase271.$===0){{return _Utils_Tuple2($l18,$elm$core$Platform$Cmd$none);}}if($tailCase271.$===1){{var $l19=$tailCase271.a;{var $l20=($elm$browser$Debugger$Main$getLatestModel)(($l18)["d"]);var $l21=A3($elm$browser$Debugger$History$add,$l19,$l20,($l18)["l"]);var $tailDestruct102_2=A2($l16,$l19,$l20);var $l22=$tailDestruct102_2.a;var $l23=$tailDestruct102_2.b;var $l24=A2($elm$core$Platform$Cmd$map,$elm$browser$Debugger$Main$UserMsg,$l23);let $tailCase101=($l18)["d"];if($tailCase101.$===0){{return _Utils_Tuple2(_Utils_update($l18,({"n":A2($elm$browser$Debugger$Expando$merge,$l22,($l18)["n"]),"l":$l21,"d":($elm$browser$Debugger$Main$Running)($l22)})),($elm$core$Platform$Cmd$batch)(_List_fromArray([$l24,($elm$browser$Debugger$Main$scroll)(($l18)["i"])])));}}{var $l25=$tailCase101.a;var $l26=$tailCase101.b;return _Utils_Tuple2(_Utils_update($l18,({"l":$l21,"d":A3($elm$browser$Debugger$Main$Paused,$l25,$l26,$l22)})),$l24);}}}}if($tailCase271.$===2){{var $l27=$tailCase271.a;return _Utils_Tuple2(_Utils_update($l18,({"n":A2($elm$browser$Debugger$Expando$update,$l27,($l18)["n"])})),$elm$core$Platform$Cmd$none);}}if($tailCase271.$===3){{let $tailCase130=($l18)["d"];if($tailCase130.$===0){{return _Utils_Tuple2($l18,$elm$core$Platform$Cmd$none);}}{var $l28=$tailCase130.c;return _Utils_Tuple2(_Utils_update($l18,({"n":A2($elm$browser$Debugger$Expando$merge,$l28,($l18)["n"]),"d":($elm$browser$Debugger$Main$Running)($l28)})),($elm$browser$Debugger$Main$scroll)(($l18)["i"]));}}}if($tailCase271.$===4){{var $l29=$tailCase271.a;{var $tailDestruct153_0=A3($elm$browser$Debugger$History$get,$l16,$l29,($l18)["l"]);var $l30=$tailDestruct153_0.a;var $l31=$tailDestruct153_0.b;return _Utils_Tuple2(_Utils_update($l18,({"n":A2($elm$browser$Debugger$Expando$merge,$l30,($l18)["n"]),"d":A3($elm$browser$Debugger$Main$Paused,$l29,$l30,($elm$browser$Debugger$Main$getLatestModel)(($l18)["d"]))})),$elm$core$Platform$Cmd$none);}}}if($tailCase271.$===5){{return _Utils_Tuple2($l18,A2($elm$core$Task$perform,(function($arg157_0){return $elm$browser$Debugger$Main$NoOp;}),(_Debugger_open)(($l18)["i"])));}}if($tailCase271.$===6){{{var $l32=(function($case171){if($case171.$===1){{var $l33=$case171.a;return $l33;}}{return ($elm$browser$Debugger$History$size)(($l18)["l"]);}})(($l18)["d"]);if((_Utils_cmp($l32,0)>0)){let $tailNext0=$l16;let $tailNext1=($elm$browser$Debugger$Main$Jump)(($l32 - 1));let $tailNext2=$l18;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}else{return _Utils_Tuple2($l18,$elm$core$Platform$Cmd$none);}}}}if($tailCase271.$===7){{let $tailCase216=($l18)["d"];if($tailCase216.$===0){{return _Utils_Tuple2($l18,$elm$core$Platform$Cmd$none);}}{var $l34=$tailCase216.a;var $l35=$tailCase216.c;if(_Utils_eq($l34,(($elm$browser$Debugger$History$size)(($l18)["l"]) - 1))){let $tailNext0=$l16;let $tailNext1=$elm$browser$Debugger$Main$Resume;let $tailNext2=$l18;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}else{let $tailNext0=$l16;let $tailNext1=($elm$browser$Debugger$Main$Jump)(($l34 + 1));let $tailNext2=$l18;$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;continue $tailLoop;}}}}if($tailCase271.$===8){{return A2($elm$browser$Debugger$Main$withGoodMetadata,$l18,(function($arg223_0){return _Utils_Tuple2($l18,$elm$browser$Debugger$Main$upload);}));}}if($tailCase271.$===9){{return A2($elm$browser$Debugger$Main$withGoodMetadata,$l18,(function($arg235_0){var $l36=$arg235_0;return _Utils_Tuple2($l18,A2($elm$browser$Debugger$Main$download,$l36,($l18)["l"]));}));}}if($tailCase271.$===10){{var $l37=$tailCase271.a;return A2($elm$browser$Debugger$Main$withGoodMetadata,$l18,(function($arg254_0){var $l38=$arg254_0;return (function($case253){if($case253.$===1){{var $l39=$case253.a;return _Utils_Tuple2(_Utils_update($l18,({"r":$l39})),$elm$core$Platform$Cmd$none);}}{var $l40=$case253.a;return A3($elm$browser$Debugger$Main$loadNewHistory,$l40,$l16,$l18);}})(A2($elm$browser$Debugger$Overlay$assessImport,$l38,$l37));}));}}{var $l41=$tailCase271.a;let $tailCase270=A2($elm$browser$Debugger$Overlay$close,$l41,($l18)["r"]);if($tailCase270.$===1){{return _Utils_Tuple2(_Utils_update($l18,({"r":$elm$browser$Debugger$Overlay$none})),$elm$core$Platform$Cmd$none);}}{var $l42=$tailCase270.a;return A3($elm$browser$Debugger$Main$loadNewHistory,$l42,$l16,$l18);}}};});
var $elm$browser$Debugger$Main$cornerView=(function($l62){return A5($elm$browser$Debugger$Overlay$view,({"ag":$elm$browser$Debugger$Main$Export,"ai":$elm$browser$Debugger$Main$Import,"al":$elm$browser$Debugger$Main$Open,"am":$elm$browser$Debugger$Main$Resume,"at":$elm$browser$Debugger$Main$OverlayMsg}),($elm$browser$Debugger$Main$isPaused)(($l62)["d"]),(_Debugger_isOpen)(($l62)["i"]),($elm$browser$Debugger$History$size)(($l62)["l"]),($l62)["r"]);;});
var $elm$browser$Debugger$Main$toBlockerType=(function($l63){return A2($elm$browser$Debugger$Overlay$toBlockerType,($elm$browser$Debugger$Main$isPaused)(($l63)["d"]),($l63)["r"]);;});
var $elm$browser$Debugger$Main$resumeStyle='\n\n.elm-debugger-resume {\n  width: 100%;\n  height: 30px;\n  line-height: 30px;\n  cursor: pointer;\n}\n\n.elm-debugger-resume:hover {\n  background-color: rgb(41, 41, 41);\n}\n\n';
var $elm$browser$Debugger$Main$viewResumeButton=(function($l74){let $tailCase595=$l74;if($tailCase595.$===1){{return ($elm$html$Html$text)('');}}{return A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Events$onClick)($elm$browser$Debugger$Main$Resume),($elm$html$Html$Attributes$class)('elm-debugger-resume')]),_List_fromArray([($elm$html$Html$text)('Resume'),A3($elm$html$Html$node,'style',_List_Nil,_List_fromArray([($elm$html$Html$text)($elm$browser$Debugger$Main$resumeStyle)]))]));};});
var $elm$browser$Debugger$Main$viewTextButton=F2(function($l72,$l73){return A2($elm$html$Html$span,_List_fromArray([($elm$html$Html$Events$onClick)($l72),A2($elm$html$Html$Attributes$style,'cursor','pointer')]),_List_fromArray([($elm$html$Html$text)($l73)]));;});
var $elm$browser$Debugger$Main$playButton=(function($l71){return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'text-align','center'),A2($elm$html$Html$Attributes$style,'background-color','rgb(50, 50, 50)')]),_List_fromArray([($elm$browser$Debugger$Main$viewResumeButton)($l71),A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'height','24px'),A2($elm$html$Html$Attributes$style,'line-height','24px'),A2($elm$html$Html$Attributes$style,'font-size','12px')]),_List_fromArray([A2($elm$browser$Debugger$Main$viewTextButton,$elm$browser$Debugger$Main$Import,'Import'),($elm$html$Html$text)(' / '),A2($elm$browser$Debugger$Main$viewTextButton,$elm$browser$Debugger$Main$Export,'Export')]))]));;});
var $elm$browser$Debugger$Main$viewSidebar=F2(function($l67,$l68){{var $l69=(function($case466){if($case466.$===0){{return $elm$core$Maybe$Nothing;}}{var $l70=$case466.a;return ($elm$core$Maybe$Just)($l70);}})($l67);return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'display','block'),A2($elm$html$Html$Attributes$style,'float','left'),A2($elm$html$Html$Attributes$style,'width','30ch'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'color','white'),A2($elm$html$Html$Attributes$style,'background-color','rgb(61, 61, 61)')]),_List_fromArray([A2($elm$html$Html$map,$elm$browser$Debugger$Main$Jump,A2($elm$browser$Debugger$History$view,$l69,$l68)),($elm$browser$Debugger$Main$playButton)($l69)]));};});
var $elm$browser$Debugger$Main$popoutView=(function($tailInput0){var $l64=$tailInput0["l"];var $l65=$tailInput0["d"];var $l66=$tailInput0["n"];return A3($elm$html$Html$node,'body',_List_fromArray([A2($elm$html$Html$Attributes$style,'margin','0'),A2($elm$html$Html$Attributes$style,'padding','0'),A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'font-family','monospace'),A2($elm$html$Html$Attributes$style,'overflow','auto')]),_List_fromArray([A2($elm$browser$Debugger$Main$viewSidebar,$l65,$l64),A2($elm$html$Html$map,$elm$browser$Debugger$Main$ExpandoMsg,A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'display','block'),A2($elm$html$Html$Attributes$style,'float','left'),A2($elm$html$Html$Attributes$style,'height','100%'),A2($elm$html$Html$Attributes$style,'width','calc(100% - 30ch)'),A2($elm$html$Html$Attributes$style,'margin','0'),A2($elm$html$Html$Attributes$style,'overflow','auto'),A2($elm$html$Html$Attributes$style,'cursor','default')]),_List_fromArray([A2($elm$browser$Debugger$Expando$view,$elm$core$Maybe$Nothing,$l66)])))]));;});
var $elm$url$Url$Url=(function($r0){return (function($r1){return (function($r2){return (function($r3){return (function($r4){return (function($r5){return ({"aB":$r5,"Y":$r1,"z":$r3,"Z":$r2,"_":$r0,"aG":$r4});});});});});});});
var $elm$url$Url$chompBeforePath=F5(function($l13,$l14,$l15,$l16,$l17){if((($elm$core$String$isEmpty)($l17)||A2($elm$core$String$contains,'@',$l17))){return $elm$core$Maybe$Nothing;}else{let $tailCase164=A2($elm$core$String$indexes,':',$l17);if(($tailCase164.$===0)){{return ($elm$core$Maybe$Just)(A6($elm$url$Url$Url,$l13,$l17,$elm$core$Maybe$Nothing,$l14,$l15,$l16));}}if(($tailCase164.$===1)&&($tailCase164.b.$===0)){{var $l18=$tailCase164.a;let $tailCase162=($elm$core$String$toInt)(A2($elm$core$String$dropLeft,($l18 + 1),$l17));if($tailCase162.$===1){{return $elm$core$Maybe$Nothing;}}{var $l19=$tailCase162;return ($elm$core$Maybe$Just)(A6($elm$url$Url$Url,$l13,A2($elm$core$String$left,$l18,$l17),$l19,$l14,$l15,$l16));}}}{return $elm$core$Maybe$Nothing;}};});
var $elm$url$Url$chompBeforeQuery=F4(function($l8,$l9,$l10,$l11){if(($elm$core$String$isEmpty)($l11)){return $elm$core$Maybe$Nothing;}else{let $tailCase115=A2($elm$core$String$indexes,'/',$l11);if(($tailCase115.$===0)){{return A5($elm$url$Url$chompBeforePath,$l8,'/',$l9,$l10,$l11);}}{var $l12=$tailCase115.a;return A5($elm$url$Url$chompBeforePath,$l8,A2($elm$core$String$dropLeft,$l12,$l11),$l9,$l10,A2($elm$core$String$left,$l12,$l11));}};});
var $elm$url$Url$chompBeforeFragment=F3(function($l4,$l5,$l6){if(($elm$core$String$isEmpty)($l6)){return $elm$core$Maybe$Nothing;}else{let $tailCase85=A2($elm$core$String$indexes,'?',$l6);if(($tailCase85.$===0)){{return A4($elm$url$Url$chompBeforeQuery,$l4,$elm$core$Maybe$Nothing,$l5,$l6);}}{var $l7=$tailCase85.a;return A4($elm$url$Url$chompBeforeQuery,$l4,($elm$core$Maybe$Just)(A2($elm$core$String$dropLeft,($l7 + 1),$l6)),$l5,A2($elm$core$String$left,$l7,$l6));}};});
var $elm$url$Url$chompAfterProtocol=F2(function($l1,$l2){if(($elm$core$String$isEmpty)($l2)){return $elm$core$Maybe$Nothing;}else{let $tailCase53=A2($elm$core$String$indexes,'#',$l2);if(($tailCase53.$===0)){{return A3($elm$url$Url$chompBeforeFragment,$l1,$elm$core$Maybe$Nothing,$l2);}}{var $l3=$tailCase53.a;return A3($elm$url$Url$chompBeforeFragment,$l1,($elm$core$Maybe$Just)(A2($elm$core$String$dropLeft,($l3 + 1),$l2)),A2($elm$core$String$left,$l3,$l2));}};});
var $elm$url$Url$fromString=(function($l0){if(A2($elm$core$String$startsWith,'http://',$l0)){return A2($elm$url$Url$chompAfterProtocol,0,A2($elm$core$String$dropLeft,7,$l0));}else{if(A2($elm$core$String$startsWith,'https://',$l0)){return A2($elm$url$Url$chompAfterProtocol,1,A2($elm$core$String$dropLeft,8,$l0));}else{return $elm$core$Maybe$Nothing;}};});
var $elm$browser$Browser$document=_Browser_document;
var $elm$browser$Browser$Internal=function(c0){return {$:0,a:c0};};
var $elm$browser$Browser$External=function(c0){return {$:1,a:c0};};
var $author$project$NotFound$page=({"v":_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)('not-found')]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','12em')]),_List_fromArray([($elm$html$Html$text)('404')])),A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','3em')]),_List_fromArray([($elm$html$Html$text)('Page not found')]))]))]),"a":'Page not found'});
var $author$project$NotFound$main=($elm$browser$Browser$document)(({"f":(function($arg4_0){return _Utils_Tuple2(0,$elm$core$Platform$Cmd$none);}),"j":(function($arg10_0){return $elm$core$Platform$Sub$none;}),"h":F2(function($arg8_0,$arg8_1){return _Utils_Tuple2(0,$elm$core$Platform$Cmd$none);}),"k":(function($arg12_0){return $author$project$NotFound$page;})}));
var $elm$browser$Browser$Dom$NotFound=function(value){return value;};
var $elm$project_metadata_utils$Elm$Error$GeneralProblem=function(c0){return {$:0,a:c0};};
var $elm$project_metadata_utils$Elm$Error$ModuleProblems=function(c0){return {$:1,a:c0};};
var $elm$project_metadata_utils$Elm$Error$toColor=(function($l4){let $tailCase171=$l4;switch($tailCase171){case 'red':{return ($elm$json$Json$Decode$succeed)(0);}case 'RED':{return ($elm$json$Json$Decode$succeed)(1);}case 'magenta':{return ($elm$json$Json$Decode$succeed)(2);}case 'MAGENTA':{return ($elm$json$Json$Decode$succeed)(3);}case 'yellow':{return ($elm$json$Json$Decode$succeed)(4);}case 'YELLOW':{return ($elm$json$Json$Decode$succeed)(5);}case 'green':{return ($elm$json$Json$Decode$succeed)(6);}case 'GREEN':{return ($elm$json$Json$Decode$succeed)(7);}case 'cyan':{return ($elm$json$Json$Decode$succeed)(8);}case 'CYAN':{return ($elm$json$Json$Decode$succeed)(9);}case 'blue':{return ($elm$json$Json$Decode$succeed)(10);}case 'BLUE':{return ($elm$json$Json$Decode$succeed)(11);}case 'white':{return ($elm$json$Json$Decode$succeed)(12);}case 'WHITE':{return ($elm$json$Json$Decode$succeed)(13);}case 'black':{return ($elm$json$Json$Decode$succeed)(14);}case 'BLACK':{return ($elm$json$Json$Decode$succeed)(15);}default:{return ($elm$json$Json$Decode$fail)(($l4+' is not a known color'));}};});
var $elm$project_metadata_utils$Elm$Error$colorDecoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Error$toColor,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Error$Style=(function($r0){return (function($r1){return (function($r2){return ({"au":$r0,"av":$r2,"aM":$r1});});});});
var $elm$project_metadata_utils$Elm$Error$Styled=F2(function(c0,c1){return {$:1,a:c0,b:c1};});
var $elm$project_metadata_utils$Elm$Error$Unstyled=function(c0){return {$:0,a:c0};};
var $elm$project_metadata_utils$Elm$Error$chunkDecoder=($elm$json$Json$Decode$oneOf)(_List_fromArray([A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Error$Unstyled,$elm$json$Json$Decode$string),A3($elm$json$Json$Decode$map2,$elm$project_metadata_utils$Elm$Error$Styled,A4($elm$json$Json$Decode$map3,$elm$project_metadata_utils$Elm$Error$Style,A2($elm$json$Json$Decode$field,'bold',$elm$json$Json$Decode$bool),A2($elm$json$Json$Decode$field,'underline',$elm$json$Json$Decode$bool),A2($elm$json$Json$Decode$field,'color',($elm$json$Json$Decode$nullable)($elm$project_metadata_utils$Elm$Error$colorDecoder))),A2($elm$json$Json$Decode$field,'string',$elm$json$Json$Decode$string))]));
var $elm$project_metadata_utils$Elm$Error$Position=(function($r0){return (function($r1){return ({"aN":$r1,"C":$r0});});});
var $elm$project_metadata_utils$Elm$Error$positionDecoder=A3($elm$json$Json$Decode$map2,$elm$project_metadata_utils$Elm$Error$Position,A2($elm$json$Json$Decode$field,'line',$elm$json$Json$Decode$int),A2($elm$json$Json$Decode$field,'column',$elm$json$Json$Decode$int));
var $elm$project_metadata_utils$Elm$Error$Region=(function($r0){return (function($r1){return ({"af":$r1,"aa":$r0});});});
var $elm$project_metadata_utils$Elm$Error$regionDecoder=A3($elm$json$Json$Decode$map2,$elm$project_metadata_utils$Elm$Error$Region,A2($elm$json$Json$Decode$field,'start',$elm$project_metadata_utils$Elm$Error$positionDecoder),A2($elm$json$Json$Decode$field,'end',$elm$project_metadata_utils$Elm$Error$positionDecoder));
var $elm$project_metadata_utils$Elm$Error$Problem=(function($r0){return (function($r1){return (function($r2){return ({"b":$r2,"aO":$r1,"a":$r0});});});});
var $elm$project_metadata_utils$Elm$Error$problemDecoder=A4($elm$json$Json$Decode$map3,$elm$project_metadata_utils$Elm$Error$Problem,A2($elm$json$Json$Decode$field,'title',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'region',$elm$project_metadata_utils$Elm$Error$regionDecoder),A2($elm$json$Json$Decode$field,'message',($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Error$chunkDecoder)));
var $elm$project_metadata_utils$Elm$Error$BadModule=(function($r0){return (function($r1){return (function($r2){return ({"m":$r1,"z":$r0,"D":$r2});});});});
var $elm$project_metadata_utils$Elm$Error$badModuleDecoder=A4($elm$json$Json$Decode$map3,$elm$project_metadata_utils$Elm$Error$BadModule,A2($elm$json$Json$Decode$field,'path',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'name',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'problems',($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Error$problemDecoder)));
var $elm$project_metadata_utils$Elm$Error$toError=(function($l0){let $tailCase47=$l0;switch($tailCase47){case 'error':{return A4($elm$json$Json$Decode$map3,F3(function($arg15_0,$arg15_1,$arg15_2){var $l1=$arg15_0;var $l2=$arg15_1;var $l3=$arg15_2;return ($elm$project_metadata_utils$Elm$Error$GeneralProblem)(({"b":$l3,"z":$l1,"a":$l2}));}),A2($elm$json$Json$Decode$field,'path',($elm$json$Json$Decode$nullable)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'title',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'message',($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Error$chunkDecoder)));}case 'compile-errors':{return A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Error$ModuleProblems,A2($elm$json$Json$Decode$field,'errors',($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Error$badModuleDecoder)));}default:{return ($elm$json$Json$Decode$fail)(($l0+' is an unknown error type'));}};});
var $elm$project_metadata_utils$Elm$Error$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Error$toError,A2($elm$json$Json$Decode$field,'type',$elm$json$Json$Decode$string));
var $author$project$Errors$colorToCss=(function($l46){let $tailCase294=$l46;switch($tailCase294){case 0:{return 'rgb(194,54,33)';}case 1:{return 'rgb(252,57,31)';}case 2:{return 'rgb(211,56,211)';}case 3:{return 'rgb(249,53,248)';}case 4:{return 'rgb(173,173,39)';}case 5:{return 'rgb(234,236,35)';}case 6:{return 'rgb(37,188,36)';}case 7:{return 'rgb(49,231,34)';}case 8:{return 'rgb(51,187,200)';}case 9:{return 'rgb(20,240,240)';}case 10:{return 'rgb(73,46,225)';}case 11:{return 'rgb(88,51,255)';}case 12:{return 'rgb(203,204,205)';}case 13:{return 'rgb(233,235,235)';}case 14:{return 'rgb(0,0,0)';}default:{return 'rgb(129,131,131)';}};});
var $author$project$Errors$addColor=F2(function($l43,$l44){let $tailCase276=$l43;if($tailCase276.$===1){{return $l44;}}{var $l45=$tailCase276.a;return A2($elm$core$List$cons,A2($elm$html$Html$Attributes$style,'color',($author$project$Errors$colorToCss)($l45)),$l44);};});
var $author$project$Errors$addUnderline=F2(function($l41,$l42){if($l41){return A2($elm$core$List$cons,A2($elm$html$Html$Attributes$style,'text-decoration','underline'),$l42);}else{return $l42;};});
var $author$project$Errors$addBold=F2(function($l39,$l40){if($l39){return A2($elm$core$List$cons,A2($elm$html$Html$Attributes$style,'font-weight','bold'),$l40);}else{return $l40;};});
var $author$project$Errors$styleToAttrs=(function($tailInput0){var $l36=$tailInput0["au"];var $l37=$tailInput0["aM"];var $l38=$tailInput0["av"];return A2($author$project$Errors$addBold,$l36,A2($author$project$Errors$addUnderline,$l37,A2($author$project$Errors$addColor,$l38,_List_Nil)));;});
var $author$project$Errors$viewMessage=(function($l29){let $tailCase236=$l29;if(($tailCase236.$===0)){{return _List_fromArray([($elm$html$Html$text)('\n\n\n')]);}}{var $l30=$tailCase236.a;var $l31=$tailCase236.b;{var $l32=(function($case229){if($case229.$===0){{var $l33=$case229.a;return ($elm$html$Html$text)($l33);}}{var $l34=$case229.a;var $l35=$case229.b;return A2($elm$html$Html$span,($author$project$Errors$styleToAttrs)($l34),_List_fromArray([($elm$html$Html$text)($l35)]));}})($l30);return A2($elm$core$List$cons,$l32,($author$project$Errors$viewMessage)($l31));}};});
var $author$project$Errors$viewSeparator=F2(function($l27,$l28){return A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'color','rgb(211,56,211)')]),_List_fromArray([($elm$html$Html$text)((A3($elm$core$String$padLeft,80,' ',($l27+'  ↑    '))+'\n'+'====o======================================================================o====\n'+'    ↓  '+$l28+'\n\n\n'))]));;});
var $author$project$Errors$fill=F2(function($l16,$l17){return _Utils_ap($l16,_Utils_ap(A2($elm$core$String$repeat,((80 - ($elm$core$String$length)($l16)) - ($elm$core$String$length)($l17)),'-'),$l17));;});
var $author$project$Errors$viewHeader=F2(function($l11,$l12){{var $l13=('-- '+$l11+' ');var $l14=(function($case111){if($case111.$===1){{return '';}}{var $l15=$case111.a;return (' '+$l15);}})($l12);return A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'color','rgb(51,187,200)')]),_List_fromArray([($elm$html$Html$text)((A2($author$project$Errors$fill,$l13,$l14)+'\n\n'))]));};});
var $author$project$Errors$viewProblem=F2(function($l25,$l26){return A2($elm$html$Html$span,_List_Nil,A2($elm$core$List$cons,A2($author$project$Errors$viewHeader,($l26)["a"],($elm$core$Maybe$Just)($l25)),($author$project$Errors$viewMessage)(($l26)["b"])));;});
var $author$project$Errors$viewBadModule=(function($tailInput0){var $l23=$tailInput0["z"];var $l24=$tailInput0["D"];return A2($elm$html$Html$span,_List_Nil,A2($elm$core$List$map,($author$project$Errors$viewProblem)($l23),$l24));;});
var $author$project$Errors$viewBadModules=(function($l18){let $tailCase164=$l18;if(($tailCase164.$===0)){{return _List_Nil;}}if(($tailCase164.$===1)&&($tailCase164.b.$===0)){{var $l19=$tailCase164.a;return _List_fromArray([($author$project$Errors$viewBadModule)($l19)]);}}{var $l20=$tailCase164.a;var $l21=$tailCase164.b.a;var $l22=$tailCase164.b.b;return A2($elm$core$List$cons,($author$project$Errors$viewBadModule)($l20),A2($elm$core$List$cons,A2($author$project$Errors$viewSeparator,($l20)["m"],($l21)["m"]),($author$project$Errors$viewBadModules)(A2($elm$core$List$cons,$l21,$l22))));};});
var $author$project$Errors$viewErrorHelp=(function($l6){let $tailCase101=$l6;if($tailCase101.$===0){{var $l7=$tailCase101.a["z"];var $l8=$tailCase101.a["a"];var $l9=$tailCase101.a["b"];return A2($elm$core$List$cons,A2($author$project$Errors$viewHeader,$l8,$l7),($author$project$Errors$viewMessage)($l9));}}{var $l10=$tailCase101.a;return ($author$project$Errors$viewBadModules)($l10);};});
var $author$project$Errors$viewError=(function($l5){return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'width','100%'),A2($elm$html$Html$Attributes$style,'min-height','100%'),A2($elm$html$Html$Attributes$style,'display','flex'),A2($elm$html$Html$Attributes$style,'flex-direction','column'),A2($elm$html$Html$Attributes$style,'align-items','center'),A2($elm$html$Html$Attributes$style,'background-color','rgb(39, 40, 34)'),A2($elm$html$Html$Attributes$style,'color','rgb(233, 235, 235)'),A2($elm$html$Html$Attributes$style,'font-family','monospace')]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'display','block'),A2($elm$html$Html$Attributes$style,'white-space','pre-wrap'),A2($elm$html$Html$Attributes$style,'background-color','black'),A2($elm$html$Html$Attributes$style,'padding','2em')]),($author$project$Errors$viewErrorHelp)($l5))]));;});
var $author$project$Errors$view=(function($l2){return ({"v":(function($case29){if($case29.$===1){{var $l3=$case29.a;return _List_fromArray([($elm$html$Html$text)(($elm$json$Json$Decode$errorToString)($l3))]);}}{var $l4=$case29.a;return _List_fromArray([($author$project$Errors$viewError)($l4)]);}})($l2),"a":'Problem!'});;});
var $author$project$Errors$main=($elm$browser$Browser$document)(({"f":(function($arg7_0){var $l1=$arg7_0;return _Utils_Tuple2(A2($elm$json$Json$Decode$decodeValue,$elm$project_metadata_utils$Elm$Error$decoder,$l1),$elm$core$Platform$Cmd$none);}),"j":(function($arg14_0){return $elm$core$Platform$Sub$none;}),"h":F2(function($arg11_0,$arg11_1){var $l0=$arg11_1;return _Utils_Tuple2($l0,$elm$core$Platform$Cmd$none);}),"k":$author$project$Errors$view}));
var $elm$svg$Svg$trustedNode=(_VirtualDom_nodeNS)('http://www.w3.org/2000/svg');
var $elm$svg$Svg$svg=($elm$svg$Svg$trustedNode)('svg');
var $elm$svg$Svg$path=($elm$svg$Svg$trustedNode)('path');
var $elm$svg$Svg$Attributes$class=(_VirtualDom_attribute)('class');
var $elm$svg$Svg$Attributes$d=(_VirtualDom_attribute)('d');
var $elm$svg$Svg$Attributes$height=(_VirtualDom_attribute)('height');
var $elm$svg$Svg$Attributes$viewBox=(_VirtualDom_attribute)('viewBox');
var $elm$svg$Svg$Attributes$width=(_VirtualDom_attribute)('width');
var $elm$svg$Svg$Attributes$fill=(_VirtualDom_attribute)('fill');
var $author$project$Index$Icon$icon=F3(function($l0,$l1,$l2){return A2($elm$svg$Svg$svg,_List_fromArray([($elm$svg$Svg$Attributes$class)('icon'),($elm$svg$Svg$Attributes$width)($l1),($elm$svg$Svg$Attributes$height)($l1),($elm$svg$Svg$Attributes$viewBox)('0 0 1792 1792')]),_List_fromArray([A2($elm$svg$Svg$path,_List_fromArray([($elm$svg$Svg$Attributes$fill)($l0),($elm$svg$Svg$Attributes$d)($l2)]),_List_Nil)]));;});
var $author$project$Index$Icon$home=A3($author$project$Index$Icon$icon,'#babdb6','36px','M1472 992v480q0 26-19 45t-45 19h-384v-384h-256v384h-384q-26 0-45-19t-19-45v-480q0-1 .5-3t.5-3l575-474 575 474q1 2 1 6zm223-69l-62 74q-8 9-21 11h-3q-13 0-21-7l-692-577-692 577q-12 8-24 7-13-2-21-11l-62-74q-8-10-7-23.5t11-21.5l719-599q32-26 76-26t76 26l244 204v-195q0-14 9-23t23-9h192q14 0 23 9t9 23v408l219 182q10 8 11 21.5t-7 23.5z');
var $author$project$Index$Icon$image=A3($author$project$Index$Icon$icon,'#babdb6','16px','M1596 380q28 28 48 76t20 88v1152q0 40-28 68t-68 28h-1344q-40 0-68-28t-28-68v-1600q0-40 28-68t68-28h896q40 0 88 20t76 48zm-444-244v376h376q-10-29-22-41l-313-313q-12-12-41-22zm384 1528v-1024h-416q-40 0-68-28t-28-68v-416h-768v1536h1280zm-128-448v320h-1024v-192l192-192 128 128 384-384zm-832-192q-80 0-136-56t-56-136 56-136 136-56 136 56 56 136-56 136-136 56z');
var $author$project$Index$Icon$file=A3($author$project$Index$Icon$icon,'#babdb6','16px','M1596 380q28 28 48 76t20 88v1152q0 40-28 68t-68 28h-1344q-40 0-68-28t-28-68v-1600q0-40 28-68t68-28h896q40 0 88 20t76 48zm-444-244v376h376q-10-29-22-41l-313-313q-12-12-41-22zm384 1528v-1024h-416q-40 0-68-28t-28-68v-416h-768v1536h1280zm-1024-864q0-14 9-23t23-9h704q14 0 23 9t9 23v64q0 14-9 23t-23 9h-704q-14 0-23-9t-9-23v-64zm736 224q14 0 23 9t9 23v64q0 14-9 23t-23 9h-704q-14 0-23-9t-9-23v-64q0-14 9-23t23-9h704zm0 256q14 0 23 9t9 23v64q0 14-9 23t-23 9h-704q-14 0-23-9t-9-23v-64q0-14 9-23t23-9h704z');
var $author$project$Index$Icon$folder=A3($author$project$Index$Icon$icon,'#babdb6','16px','M1728 608v704q0 92-66 158t-158 66h-1216q-92 0-158-66t-66-158v-960q0-92 66-158t158-66h320q92 0 158 66t66 158v32h672q92 0 158 66t66 158z');
var $author$project$Index$Icon$package=A3($author$project$Index$Icon$icon,'#babdb6','16px','M1088 832q0-26-19-45t-45-19h-256q-26 0-45 19t-19 45 19 45 45 19h256q26 0 45-19t19-45zm576-192v960q0 26-19 45t-45 19h-1408q-26 0-45-19t-19-45v-960q0-26 19-45t45-19h1408q26 0 45 19t19 45zm64-448v256q0 26-19 45t-45 19h-1536q-26 0-45-19t-19-45v-256q0-26 19-45t45-19h1536q26 0 45 19t19 45z');
var $author$project$Index$Icon$plus=A3($author$project$Index$Icon$icon,'#babdb6','16px','M1600 736v192q0 40-28 68t-68 28h-416v416q0 40-28 68t-68 28h-192q-40 0-68-28t-28-68v-416h-416q-40 0-68-28t-28-68v-192q0-40 28-68t68-28h416v-416q0-40 28-68t68-28h192q40 0 68 28t28 68v416h416q40 0 68 28t28 68z');
var $author$project$Index$Icon$getExtensionHelp=(function($tailInput0){let $tailState0=$tailInput0;$tailLoop:while(true){var $l6=$tailState0;let $tailCase101=$l6;if(($tailCase101.$===0)){{return '';}}if(($tailCase101.$===1)&&($tailCase101.b.$===0)){{var $l7=$tailCase101.a;return ($elm$core$String$toLower)($l7);}}{var $l8=$tailCase101.b;let $tailNext0=$l8;$tailState0=$tailNext0;continue $tailLoop;}};});
var $author$project$Index$Icon$getExtension=(function($l5){return ($author$project$Index$Icon$getExtensionHelp)(A2($elm$core$String$split,'.',$l5));;});
var $author$project$Index$Icon$extensionIcons=($elm$core$Dict$fromList)(_List_fromArray([_Utils_Tuple2('jpg',$author$project$Index$Icon$image),_Utils_Tuple2('jpeg',$author$project$Index$Icon$image),_Utils_Tuple2('png',$author$project$Index$Icon$image),_Utils_Tuple2('gif',$author$project$Index$Icon$image)]));
var $author$project$Index$Icon$lookup=(function($l3){{var $l4=($author$project$Index$Icon$getExtension)($l3);return A2($elm$core$Maybe$withDefault,$author$project$Index$Icon$file,A2($elm$core$Dict$get,$l4,$author$project$Index$Icon$extensionIcons));};});
var $author$project$Index$Navigator$slash=A2($elm$html$Html$span,_List_fromArray([A2($elm$html$Html$Attributes$style,'padding','0 8px')]),_List_fromArray([($elm$html$Html$text)('/')]));
var $author$project$Index$Navigator$addSlash=F2(function($l13,$l14){return A2($elm$core$List$cons,$l13,A2($elm$core$List$cons,$author$project$Index$Navigator$slash,$l14));;});
var $author$project$Index$Navigator$makeLinks=F4(function($tailInput0,$tailInput1,$tailInput2,$tailInput3){let $tailState0=$tailInput0;let $tailState1=$tailInput1;let $tailState2=$tailInput2;let $tailState3=$tailInput3;$tailLoop:while(true){var $l2=$tailState0;var $l3=$tailState1;var $l4=$tailState2;var $l5=$tailState3;let $tailCase82=$l3;if(($tailCase82.$===1)){{var $l6=$tailCase82.a;var $l7=$tailCase82.b;{var $l8=($l4+'/'+$l6);var $l9=A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)($l8)]),_List_fromArray([($elm$html$Html$text)($l6)]));let $tailNext0=$l2;let $tailNext1=$l7;let $tailNext2=$l8;let $tailNext3=A2($elm$core$List$cons,$l9,$l5);$tailState0=$tailNext0;$tailState1=$tailNext1;$tailState2=$tailNext2;$tailState3=$tailNext3;continue $tailLoop;}}}{{var $l10=A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)('/'),($elm$html$Html$Attributes$title)($l2),A2($elm$html$Html$Attributes$style,'display','inherit')]),_List_fromArray([$author$project$Index$Icon$home]));let $tailCase80=$l5;if(($tailCase80.$===0)){{return _List_fromArray([$l10]);}}{var $l11=$tailCase80.a;var $l12=$tailCase80.b;return A2($elm$core$List$cons,$l10,A2($elm$core$List$cons,$author$project$Index$Navigator$slash,A3($elm$core$List$foldl,$author$project$Index$Navigator$addSlash,_List_fromArray([$l11]),$l12)));}}}};});
var $author$project$Index$Navigator$view=F2(function($l0,$l1){return A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'font-size','2em'),A2($elm$html$Html$Attributes$style,'padding','20px 0'),A2($elm$html$Html$Attributes$style,'display','flex'),A2($elm$html$Html$Attributes$style,'align-items','center'),A2($elm$html$Html$Attributes$style,'height','40px')]),A4($author$project$Index$Navigator$makeLinks,$l0,$l1,'',_List_Nil));;});
var $elm_explorations$markdown$Markdown$toHtmlWith=_Markdown_toHtml;
var $elm_explorations$markdown$Markdown$defaultOptions=({"X":$elm$core$Maybe$Nothing,"ah":($elm$core$Maybe$Just)(({"ac":false,"ap":false})),"an":true,"ao":false});
var $elm_explorations$markdown$Markdown$toHtml=($elm_explorations$markdown$Markdown$toHtmlWith)($elm_explorations$markdown$Markdown$defaultOptions);
var $author$project$Index$Skeleton$boxFooter=(function($l9){let $tailCase73=$l9;if($tailCase73.$===1){{return ($elm$html$Html$text)('');}}{var $l10=$tailCase73.a.a;var $l11=$tailCase73.a.b;return A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)($l10),($elm$html$Html$Attributes$title)($l11)]),_List_fromArray([A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)('box-footer')]),_List_fromArray([$author$project$Index$Icon$plus]))]));};});
var $author$project$Index$Skeleton$boxHelp=F3(function($l6,$l7,$l8){return A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)('box')]),A2($elm$core$List$cons,A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)('box-header')]),_List_fromArray([($elm$html$Html$text)($l6)])),_Utils_ap($l7,_List_fromArray([($author$project$Index$Skeleton$boxFooter)($l8)]))));;});
var $author$project$Index$Skeleton$box=(function($tailInput0){var $l0=$tailInput0["a"];var $l1=$tailInput0["x"];var $l2=$tailInput0["w"];{var $l3=A2($elm$core$List$map,($elm$html$Html$div)(_List_fromArray([($elm$html$Html$Attributes$class)('box-item')])),$l1);return A3($author$project$Index$Skeleton$boxHelp,$l0,$l3,$l2);};});
var $author$project$Index$Skeleton$readmeBox=(function($l4){{var $l5=A2($elm_explorations$markdown$Markdown$toHtml,_List_fromArray([($elm$html$Html$Attributes$class)('box-item')]),$l4);return A3($author$project$Index$Skeleton$boxHelp,'README',_List_fromArray([$l5]),$elm$core$Maybe$Nothing);};});
var $elm$project_metadata_utils$Elm$License$License=F2(function(c0,c1){return {$:0,a:c0,b:c1};});
var $elm$project_metadata_utils$Elm$License$toString=(function($tailInput0){var $l0=$tailInput0.a;return $l0;;});
var $elm$project_metadata_utils$Elm$License$osiApprovedSpdxLicenses=_List_fromArray([A2($elm$project_metadata_utils$Elm$License$License,'AFL-1.1','Academic Free License v1.1'),A2($elm$project_metadata_utils$Elm$License$License,'AFL-1.2','Academic Free License v1.2'),A2($elm$project_metadata_utils$Elm$License$License,'AFL-2.0','Academic Free License v2.0'),A2($elm$project_metadata_utils$Elm$License$License,'AFL-2.1','Academic Free License v2.1'),A2($elm$project_metadata_utils$Elm$License$License,'AFL-3.0','Academic Free License v3.0'),A2($elm$project_metadata_utils$Elm$License$License,'APL-1.0','Adaptive Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'Apache-1.1','Apache License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'Apache-2.0','Apache License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'APSL-1.0','Apple Public Source License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'APSL-1.1','Apple Public Source License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'APSL-1.2','Apple Public Source License 1.2'),A2($elm$project_metadata_utils$Elm$License$License,'APSL-2.0','Apple Public Source License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'Artistic-1.0','Artistic License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'Artistic-1.0-Perl','Artistic License 1.0 (Perl)'),A2($elm$project_metadata_utils$Elm$License$License,'Artistic-1.0-cl8','Artistic License 1.0 w/clause 8'),A2($elm$project_metadata_utils$Elm$License$License,'Artistic-2.0','Artistic License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'AAL','Attribution Assurance License'),A2($elm$project_metadata_utils$Elm$License$License,'BSL-1.0','Boost Software License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'BSD-2-Clause','BSD 2-clause \"Simplified\" License'),A2($elm$project_metadata_utils$Elm$License$License,'BSD-3-Clause','BSD 3-clause \"New\" or \"Revised\" License'),A2($elm$project_metadata_utils$Elm$License$License,'0BSD','BSD Zero Clause License'),A2($elm$project_metadata_utils$Elm$License$License,'CECILL-2.1','CeCILL Free Software License Agreement v2.1'),A2($elm$project_metadata_utils$Elm$License$License,'CNRI-Python','CNRI Python License'),A2($elm$project_metadata_utils$Elm$License$License,'CDDL-1.0','Common Development and Distribution License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'CPAL-1.0','Common Public Attribution License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'CPL-1.0','Common Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'CATOSL-1.1','Computer Associates Trusted Open Source License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'CUA-OPL-1.0','CUA Office Public License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'EPL-1.0','Eclipse Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'ECL-1.0','Educational Community License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'ECL-2.0','Educational Community License v2.0'),A2($elm$project_metadata_utils$Elm$License$License,'EFL-1.0','Eiffel Forum License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'EFL-2.0','Eiffel Forum License v2.0'),A2($elm$project_metadata_utils$Elm$License$License,'Entessa','Entessa Public License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'EUDatagrid','EU DataGrid Software License'),A2($elm$project_metadata_utils$Elm$License$License,'EUPL-1.1','European Union Public License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'Fair','Fair License'),A2($elm$project_metadata_utils$Elm$License$License,'Frameworx-1.0','Frameworx Open License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'AGPL-3.0','GNU Affero General Public License v3.0'),A2($elm$project_metadata_utils$Elm$License$License,'GPL-2.0','GNU General Public License v2.0 only'),A2($elm$project_metadata_utils$Elm$License$License,'GPL-3.0','GNU General Public License v3.0 only'),A2($elm$project_metadata_utils$Elm$License$License,'LGPL-2.1','GNU Lesser General Public License v2.1 only'),A2($elm$project_metadata_utils$Elm$License$License,'LGPL-3.0','GNU Lesser General Public License v3.0 only'),A2($elm$project_metadata_utils$Elm$License$License,'LGPL-2.0','GNU Library General Public License v2 only'),A2($elm$project_metadata_utils$Elm$License$License,'HPND','Historic Permission Notice and Disclaimer'),A2($elm$project_metadata_utils$Elm$License$License,'IPL-1.0','IBM Public License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'Intel','Intel Open Source License'),A2($elm$project_metadata_utils$Elm$License$License,'IPA','IPA Font License'),A2($elm$project_metadata_utils$Elm$License$License,'ISC','ISC License'),A2($elm$project_metadata_utils$Elm$License$License,'LPPL-1.3c','LaTeX Project Public License v1.3c'),A2($elm$project_metadata_utils$Elm$License$License,'LiLiQ-P-1.1','Licence Libre du Québec – Permissive version 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'LiLiQ-Rplus-1.1','Licence Libre du Québec – Réciprocité forte version 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'LiLiQ-R-1.1','Licence Libre du Québec – Réciprocité version 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'LPL-1.02','Lucent Public License v1.02'),A2($elm$project_metadata_utils$Elm$License$License,'LPL-1.0','Lucent Public License Version 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'MS-PL','Microsoft Public License'),A2($elm$project_metadata_utils$Elm$License$License,'MS-RL','Microsoft Reciprocal License'),A2($elm$project_metadata_utils$Elm$License$License,'MirOS','MirOS Licence'),A2($elm$project_metadata_utils$Elm$License$License,'MIT','MIT License'),A2($elm$project_metadata_utils$Elm$License$License,'Motosoto','Motosoto License'),A2($elm$project_metadata_utils$Elm$License$License,'MPL-1.0','Mozilla Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'MPL-1.1','Mozilla Public License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'MPL-2.0','Mozilla Public License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'MPL-2.0-no-copyleft-exception','Mozilla Public License 2.0 (no copyleft exception)'),A2($elm$project_metadata_utils$Elm$License$License,'Multics','Multics License'),A2($elm$project_metadata_utils$Elm$License$License,'NASA-1.3','NASA Open Source Agreement 1.3'),A2($elm$project_metadata_utils$Elm$License$License,'Naumen','Naumen Public License'),A2($elm$project_metadata_utils$Elm$License$License,'NGPL','Nethack General Public License'),A2($elm$project_metadata_utils$Elm$License$License,'Nokia','Nokia Open Source License'),A2($elm$project_metadata_utils$Elm$License$License,'NPOSL-3.0','Non-Profit Open Software License 3.0'),A2($elm$project_metadata_utils$Elm$License$License,'NTP','NTP License'),A2($elm$project_metadata_utils$Elm$License$License,'OCLC-2.0','OCLC Research Public License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'OGTSL','Open Group Test Suite License'),A2($elm$project_metadata_utils$Elm$License$License,'OSL-1.0','Open Software License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'OSL-2.0','Open Software License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'OSL-2.1','Open Software License 2.1'),A2($elm$project_metadata_utils$Elm$License$License,'OSL-3.0','Open Software License 3.0'),A2($elm$project_metadata_utils$Elm$License$License,'OSET-PL-2.1','OSET Public License version 2.1'),A2($elm$project_metadata_utils$Elm$License$License,'PHP-3.0','PHP License v3.0'),A2($elm$project_metadata_utils$Elm$License$License,'PostgreSQL','PostgreSQL License'),A2($elm$project_metadata_utils$Elm$License$License,'Python-2.0','Python License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'QPL-1.0','Q Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'RPSL-1.0','RealNetworks Public Source License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'RPL-1.1','Reciprocal Public License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'RPL-1.5','Reciprocal Public License 1.5'),A2($elm$project_metadata_utils$Elm$License$License,'RSCPL','Ricoh Source Code Public License'),A2($elm$project_metadata_utils$Elm$License$License,'OFL-1.1','SIL Open Font License 1.1'),A2($elm$project_metadata_utils$Elm$License$License,'SimPL-2.0','Simple Public License 2.0'),A2($elm$project_metadata_utils$Elm$License$License,'Sleepycat','Sleepycat License'),A2($elm$project_metadata_utils$Elm$License$License,'SISSL','Sun Industry Standards Source License v1.1'),A2($elm$project_metadata_utils$Elm$License$License,'SPL-1.0','Sun Public License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'Watcom-1.0','Sybase Open Watcom Public License 1.0'),A2($elm$project_metadata_utils$Elm$License$License,'UPL-1.0','Universal Permissive License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'NCSA','University of Illinois/NCSA Open Source License'),A2($elm$project_metadata_utils$Elm$License$License,'VSL-1.0','Vovida Software License v1.0'),A2($elm$project_metadata_utils$Elm$License$License,'W3C','W3C Software Notice and License (2002-12-31)'),A2($elm$project_metadata_utils$Elm$License$License,'Xnet','X.Net License'),A2($elm$project_metadata_utils$Elm$License$License,'Zlib','zlib License'),A2($elm$project_metadata_utils$Elm$License$License,'ZPL-2.0','Zope Public License 2.0')]);
var $elm$project_metadata_utils$Elm$License$spdxDict=($elm$core$Dict$fromList)(A2($elm$core$List$map,(function($arg14_0){var $l2=$arg14_0;var $l3=$arg14_0.a;return _Utils_Tuple2($l3,$l2);}),$elm$project_metadata_utils$Elm$License$osiApprovedSpdxLicenses));
var $elm$project_metadata_utils$Elm$License$fromString=(function($l1){return A2($elm$core$Dict$get,$l1,$elm$project_metadata_utils$Elm$License$spdxDict);;});
var $elm$project_metadata_utils$Elm$License$decoderHelp=(function($l5){let $tailCase36=($elm$project_metadata_utils$Elm$License$fromString)($l5);if($tailCase36.$===0){{var $l6=$tailCase36.a;return ($elm$json$Json$Decode$succeed)($l6);}}{return ($elm$json$Json$Decode$fail)('I need an OSI approved license in SPDX format <https://spdx.org/licenses/>');};});
var $elm$project_metadata_utils$Elm$License$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$License$decoderHelp,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Package$Name=F2(function(c0,c1){return {$:0,a:c0,b:c1};});
var $elm$project_metadata_utils$Elm$Package$toString=(function($tailInput0){var $l0=$tailInput0.a;var $l1=$tailInput0.b;return ($l0+'/'+$l1);;});
var $elm$project_metadata_utils$Elm$Package$isBadChar=(function($l7){return (($elm$core$Char$isUpper)($l7)||(_Utils_eq($l7,'.')||_Utils_eq($l7,'_')));;});
var $elm$project_metadata_utils$Elm$Package$isBadProjectName=(function($l5){let $tailCase43=($elm$core$String$uncons)($l5);if($tailCase43.$===1){{return true;}}{var $l6=$tailCase43.a.a;return (A2($elm$core$String$contains,'--',$l5)||(A2($elm$core$String$any,$elm$project_metadata_utils$Elm$Package$isBadChar,$l5)||(A2($elm$core$String$startsWith,'-',$l5)||(!($elm$core$Char$isLower)($l6)))));};});
var $elm$project_metadata_utils$Elm$Package$fromString=(function($l2){let $tailCase20=A2($elm$core$String$split,'/',$l2);if(($tailCase20.$===1)&&($tailCase20.b.$===1)&&($tailCase20.b.b.$===0)){{var $l3=$tailCase20.a;var $l4=$tailCase20.b.a;if(($elm$project_metadata_utils$Elm$Package$isBadProjectName)($l4)){return $elm$core$Maybe$Nothing;}else{return ($elm$core$Maybe$Just)(A2($elm$project_metadata_utils$Elm$Package$Name,$l3,$l4));}}}{return $elm$core$Maybe$Nothing;};});
var $elm$project_metadata_utils$Elm$Package$decoderHelp=(function($l9){let $tailCase70=($elm$project_metadata_utils$Elm$Package$fromString)($l9);if($tailCase70.$===0){{var $l10=$tailCase70.a;return ($elm$json$Json$Decode$succeed)($l10);}}{return ($elm$json$Json$Decode$fail)('I need a valid package name like \"elm/core\"');};});
var $elm$project_metadata_utils$Elm$Package$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Package$decoderHelp,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Version$Version=F3(function(c0,c1,c2){return {$:0,a:c0,b:c1,c:c2};});
var $elm$project_metadata_utils$Elm$Version$compare=F2(function($tailInput0,$tailInput1){var $l0=$tailInput0.a;var $l1=$tailInput0.b;var $l2=$tailInput0.c;var $l3=$tailInput1.a;var $l4=$tailInput1.b;var $l5=$tailInput1.c;let $tailCase22=A2($elm$core$Basics$compare,$l0,$l3);switch($tailCase22){case 0:{return 0;}case 2:{return 2;}default:{let $tailCase21=A2($elm$core$Basics$compare,$l1,$l4);switch($tailCase21){case 0:{return 0;}case 1:{return A2($elm$core$Basics$compare,$l2,$l5);}default:{return 2;}}}};});
var $elm$project_metadata_utils$Elm$Version$toString=(function($tailInput0){var $l6=$tailInput0.a;var $l7=$tailInput0.b;var $l8=$tailInput0.c;return (($elm$core$String$fromInt)($l6)+'.'+($elm$core$String$fromInt)($l7)+'.'+($elm$core$String$fromInt)($l8));;});
var $elm$project_metadata_utils$Elm$Version$checkNumbers=F3(function($l13,$l14,$l15){if(((_Utils_cmp($l13,0)>-1)&&((_Utils_cmp($l14,0)>-1)&&(_Utils_cmp($l15,0)>-1)))){return ($elm$core$Maybe$Just)(A3($elm$project_metadata_utils$Elm$Version$Version,$l13,$l14,$l15));}else{return $elm$core$Maybe$Nothing;};});
var $elm$project_metadata_utils$Elm$Version$fromString=(function($l9){let $tailCase48=A2($elm$core$List$map,$elm$core$String$toInt,A2($elm$core$String$split,'.',$l9));if(($tailCase48.$===1)&&($tailCase48.b.$===1)&&($tailCase48.b.b.$===1)&&($tailCase48.b.b.b.$===0)&&$tailCase48.a.$===0&&$tailCase48.b.a.$===0&&$tailCase48.b.b.a.$===0){{var $l10=$tailCase48.a.a;var $l11=$tailCase48.b.a.a;var $l12=$tailCase48.b.b.a.a;return A3($elm$project_metadata_utils$Elm$Version$checkNumbers,$l10,$l11,$l12);}}{return $elm$core$Maybe$Nothing;};});
var $elm$project_metadata_utils$Elm$Version$decoderHelp=(function($l23){let $tailCase92=($elm$project_metadata_utils$Elm$Version$fromString)($l23);if($tailCase92.$===0){{var $l24=$tailCase92.a;return ($elm$json$Json$Decode$succeed)($l24);}}{return ($elm$json$Json$Decode$fail)('I need a valid version like \"2.0.1\"');};});
var $elm$project_metadata_utils$Elm$Version$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Version$decoderHelp,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Constraint$Constraint=F4(function(c0,c1,c2,c3){return {$:0,a:c0,b:c1,c:c2,d:c3};});
var $elm$project_metadata_utils$Elm$Constraint$checkConstraint=(function($tailInput0){var $l19=$tailInput0;var $l20=$tailInput0.a;var $l21=$tailInput0.d;let $tailCase85=A2($elm$project_metadata_utils$Elm$Version$compare,$l20,$l21);switch($tailCase85){case 0:{return ($elm$core$Maybe$Just)($l19);}case 1:{return ($elm$core$Maybe$Just)($l19);}default:{return $elm$core$Maybe$Nothing;}};});
var $elm$project_metadata_utils$Elm$Constraint$opFromString=(function($l18){let $tailCase73=$l18;switch($tailCase73){case '<':{return ($elm$core$Maybe$Just)(0);}case '<=':{return ($elm$core$Maybe$Just)(1);}default:{return $elm$core$Maybe$Nothing;}};});
var $elm$project_metadata_utils$Elm$Constraint$fromString=(function($l13){let $tailCase64=A2($elm$core$String$split,' ',$l13);if(($tailCase64.$===1)&&($tailCase64.b.$===1)&&($tailCase64.b.b.$===1)&&($tailCase64.b.b.b.$===1)&&($tailCase64.b.b.b.b.$===1)&&($tailCase64.b.b.b.b.b.$===0)&&($tailCase64.b.b.a==='v')){{var $l14=$tailCase64.a;var $l15=$tailCase64.b.a;var $l16=$tailCase64.b.b.b.a;var $l17=$tailCase64.b.b.b.b.a;return A2($elm$core$Maybe$andThen,$elm$project_metadata_utils$Elm$Constraint$checkConstraint,A5($elm$core$Maybe$map4,$elm$project_metadata_utils$Elm$Constraint$Constraint,($elm$project_metadata_utils$Elm$Version$fromString)($l14),($elm$project_metadata_utils$Elm$Constraint$opFromString)($l15),($elm$project_metadata_utils$Elm$Constraint$opFromString)($l16),($elm$project_metadata_utils$Elm$Version$fromString)($l17)));}}{return $elm$core$Maybe$Nothing;};});
var $elm$project_metadata_utils$Elm$Constraint$decoderHelp=(function($l23){let $tailCase104=($elm$project_metadata_utils$Elm$Constraint$fromString)($l23);if($tailCase104.$===0){{var $l24=$tailCase104.a;return ($elm$json$Json$Decode$succeed)($l24);}}{return ($elm$json$Json$Decode$fail)('I need a valid constraint like \"1.0.0 <= v < 2.0.0\"');};});
var $elm$project_metadata_utils$Elm$Constraint$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Constraint$decoderHelp,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Module$Name=function(value){return value;};
var $elm$project_metadata_utils$Elm$Module$isGoodChunk=(function($l2){let $tailCase27=($elm$core$String$uncons)($l2);if($tailCase27.$===1){{return false;}}{var $l3=$tailCase27.a.a;var $l4=$tailCase27.a.b;return (($elm$core$Char$isUpper)($l3)&&A2($elm$core$String$all,$elm$core$Char$isAlpha,$l4));};});
var $elm$project_metadata_utils$Elm$Module$fromString=(function($l1){if(A2($elm$core$List$all,$elm$project_metadata_utils$Elm$Module$isGoodChunk,A2($elm$core$String$split,'.',$l1))){return ($elm$core$Maybe$Just)(($elm$project_metadata_utils$Elm$Module$Name)($l1));}else{return $elm$core$Maybe$Nothing;};});
var $elm$project_metadata_utils$Elm$Module$decoderHelp=(function($l6){let $tailCase44=($elm$project_metadata_utils$Elm$Module$fromString)($l6);if($tailCase44.$===0){{var $l7=$tailCase44.a;return ($elm$json$Json$Decode$succeed)($l7);}}{return ($elm$json$Json$Decode$fail)('I need a valid module name like \"Json.Decode\"');};});
var $elm$project_metadata_utils$Elm$Module$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Module$decoderHelp,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Project$Application=function(c0){return {$:0,a:c0};};
var $elm$project_metadata_utils$Elm$Project$Package=function(c0){return {$:1,a:c0};};
var $elm$project_metadata_utils$Elm$Project$ApplicationInfo=(function($r0){return (function($r1){return (function($r2){return (function($r3){return (function($r4){return (function($r5){return ({"ae":$r2,"aw":$r3,"L":$r1,"q":$r0,"ar":$r4,"aL":$r5});});});});});});});
var $elm$project_metadata_utils$Elm$Project$PackageInfo=(function($r0){return (function($r1){return (function($r2){return (function($r3){return (function($r4){return (function($r5){return (function($r6){return (function($r7){return ({"ad":$r5,"q":$r7,"az":$r4,"aj":$r2,"m":$r0,"aK":$r1,"aq":$r6,"as":$r3});});});});});});});});});
var $elm$project_metadata_utils$Elm$Project$ExposedList=function(c0){return {$:0,a:c0};};
var $elm$project_metadata_utils$Elm$Project$ExposedDict=function(c0){return {$:1,a:c0};};
var $elm$project_metadata_utils$Elm$Project$checkHeaders=(function($tailInput0){let $tailState0=$tailInput0;$tailLoop:while(true){var $l37=$tailState0;let $tailCase345=$l37;if(($tailCase345.$===0)){{return $elm$core$Maybe$Nothing;}}{var $l38=$tailCase345.a.a;var $l39=$tailCase345.b;if((_Utils_cmp(($elm$core$String$length)($l38),20)<0)){let $tailNext0=$l39;$tailState0=$tailNext0;continue $tailLoop;}else{return ($elm$core$Maybe$Just)($l38);}}};});
var $elm$project_metadata_utils$Elm$Project$checkExposedDict=(function($l35){let $tailCase330=($elm$project_metadata_utils$Elm$Project$checkHeaders)($l35);if($tailCase330.$===1){{return ($elm$json$Json$Decode$succeed)($l35);}}{var $l36=$tailCase330.a;return ($elm$json$Json$Decode$fail)(('The \"'+$l36+'\" header is too long. Twenty characters max!'));};});
var $elm$project_metadata_utils$Elm$Project$exposedDecoder=($elm$json$Json$Decode$oneOf)(_List_fromArray([A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Project$ExposedList,($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Module$decoder)),A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Project$ExposedDict,A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Project$checkExposedDict,($elm$json$Json$Decode$keyValuePairs)(($elm$json$Json$Decode$list)($elm$project_metadata_utils$Elm$Module$decoder))))]));
var $elm$project_metadata_utils$Elm$Project$verifyDepNames=F2(function($tailInput0,$tailInput1){let $tailState0=$tailInput0;let $tailState1=$tailInput1;$tailLoop:while(true){var $l29=$tailState0;var $l30=$tailState1;let $tailCase296=$l30;if(($tailCase296.$===0)){{return ($elm$json$Json$Decode$succeed)(($elm$core$List$reverse)($l29));}}{var $l31=$tailCase296.a.a;var $l32=$tailCase296.a.b;var $l33=$tailCase296.b;let $tailCase295=($elm$project_metadata_utils$Elm$Package$fromString)($l31);if($tailCase295.$===0){{var $l34=$tailCase295.a;let $tailNext0=A2($elm$core$List$cons,_Utils_Tuple2($l34,$l32),$l29);let $tailNext1=$l33;$tailState0=$tailNext0;$tailState1=$tailNext1;continue $tailLoop;}}{return ($elm$json$Json$Decode$fail)(('\"'+$l31+'\" is not a valid package name.'));}}};});
var $elm$project_metadata_utils$Elm$Project$depsDecoder=(function($l28){return A2($elm$json$Json$Decode$andThen,($elm$project_metadata_utils$Elm$Project$verifyDepNames)(_List_Nil),($elm$json$Json$Decode$keyValuePairs)($l28));;});
var $elm$project_metadata_utils$Elm$Project$summaryCheck=(function($l27){if((_Utils_cmp(($elm$core$String$length)($l27),80)<0)){return ($elm$json$Json$Decode$succeed)($l27);}else{return ($elm$json$Json$Decode$fail)('The \"summary\" field must have fewer than 80 characters.');};});
var $elm$project_metadata_utils$Elm$Project$summaryDecoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Project$summaryCheck,$elm$json$Json$Decode$string);
var $elm$project_metadata_utils$Elm$Project$packageDecoder=A9($elm$json$Json$Decode$map8,$elm$project_metadata_utils$Elm$Project$PackageInfo,A2($elm$json$Json$Decode$field,'name',$elm$project_metadata_utils$Elm$Package$decoder),A2($elm$json$Json$Decode$field,'summary',$elm$project_metadata_utils$Elm$Project$summaryDecoder),A2($elm$json$Json$Decode$field,'license',$elm$project_metadata_utils$Elm$License$decoder),A2($elm$json$Json$Decode$field,'version',$elm$project_metadata_utils$Elm$Version$decoder),A2($elm$json$Json$Decode$field,'exposed-modules',$elm$project_metadata_utils$Elm$Project$exposedDecoder),A2($elm$json$Json$Decode$field,'dependencies',($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Constraint$decoder)),A2($elm$json$Json$Decode$field,'test-dependencies',($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Constraint$decoder)),A2($elm$json$Json$Decode$field,'elm-version',$elm$project_metadata_utils$Elm$Constraint$decoder));
var $elm$project_metadata_utils$Elm$Project$applicationDecoder=A7($elm$json$Json$Decode$map6,$elm$project_metadata_utils$Elm$Project$ApplicationInfo,A2($elm$json$Json$Decode$field,'elm-version',$elm$project_metadata_utils$Elm$Version$decoder),A2($elm$json$Json$Decode$field,'source-directories',($elm$json$Json$Decode$list)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$at,_List_fromArray(['dependencies','direct']),($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Version$decoder)),A2($elm$json$Json$Decode$at,_List_fromArray(['dependencies','indirect']),($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Version$decoder)),A2($elm$json$Json$Decode$at,_List_fromArray(['test-dependencies','direct']),($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Version$decoder)),A2($elm$json$Json$Decode$at,_List_fromArray(['test-dependencies','indirect']),($elm$project_metadata_utils$Elm$Project$depsDecoder)($elm$project_metadata_utils$Elm$Version$decoder)));
var $elm$project_metadata_utils$Elm$Project$decoderHelp=(function($l25){let $tailCase163=$l25;switch($tailCase163){case 'application':{return A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Project$Application,$elm$project_metadata_utils$Elm$Project$applicationDecoder);}case 'package':{return A2($elm$json$Json$Decode$map,$elm$project_metadata_utils$Elm$Project$Package,$elm$project_metadata_utils$Elm$Project$packageDecoder);}default:{var $l26=$tailCase163;return ($elm$json$Json$Decode$fail)(('The "type" field must be either "application" or "package", so '+'\"'+$l26+'\" is not acceptable.'));}};});
var $elm$project_metadata_utils$Elm$Project$decoder=A2($elm$json$Json$Decode$andThen,$elm$project_metadata_utils$Elm$Project$decoderHelp,A2($elm$json$Json$Decode$field,'type',$elm$json$Json$Decode$string));
var $author$project$Index$toPackageUrl=F2(function($l42,$l43){return ('https://package.elm-lang.org/packages/'+($elm$project_metadata_utils$Elm$Package$toString)($l42)+'/'+($elm$project_metadata_utils$Elm$Version$toString)($l43));;});
var $author$project$Index$viewVersion=(function($tailInput0){var $l37=$tailInput0.a;var $l38=$tailInput0.b;return _List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'float','left')]),_List_fromArray([$author$project$Index$Icon$package,A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)(A2($author$project$Index$toPackageUrl,$l37,$l38))]),_List_fromArray([($elm$html$Html$text)(($elm$project_metadata_utils$Elm$Package$toString)($l37))]))])),A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'float','right')]),_List_fromArray([($elm$html$Html$text)(($elm$project_metadata_utils$Elm$Version$toString)($l38))]))]);;});
var $author$project$Index$viewConstraint=F2(function($l39,$tailInput1){var $l40=$tailInput1.a;let $tailCase398=A2($elm$core$Dict$get,($elm$project_metadata_utils$Elm$Package$toString)($l40),$l39);if($tailCase398.$===0){{var $l41=$tailCase398.a;return ($author$project$Index$viewVersion)(_Utils_Tuple2($l40,$l41));}}{return _List_fromArray([A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'float','left')]),_List_fromArray([$author$project$Index$Icon$package,($elm$html$Html$text)(($elm$project_metadata_utils$Elm$Package$toString)($l40))])),A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'float','right')]),_List_fromArray([($elm$html$Html$text)('???')]))]);};});
var $author$project$Index$viewTestDeps=F2(function($l32,$l33){{var $l34=(function($case315){if($case315.$===0){{var $l35=$case315.a;return A2($elm$core$List$map,$author$project$Index$viewVersion,($l35)["ar"]);}}{var $l36=$case315.a;return A2($elm$core$List$map,($author$project$Index$viewConstraint)($l32),($l36)["aq"]);}})($l33);return ($author$project$Index$Skeleton$box)(({"w":$elm$core$Maybe$Nothing,"x":$l34,"a":'Test Dependencies'}));};});
var $author$project$Index$viewDeps=F2(function($l27,$l28){{var $l29=(function($case294){if($case294.$===0){{var $l30=$case294.a;return A2($elm$core$List$map,$author$project$Index$viewVersion,($l30)["ae"]);}}{var $l31=$case294.a;return A2($elm$core$List$map,($author$project$Index$viewConstraint)($l27),($l31)["ad"]);}})($l28);return ($author$project$Index$Skeleton$box)(({"w":$elm$core$Maybe$Nothing,"x":$l29,"a":'Dependencies'}));};});
var $author$project$Index$viewProjectSummary=(function($l23){let $tailCase280=$l23;if($tailCase280.$===0){{var $l24=$tailCase280.a;return ($author$project$Index$Skeleton$box)(({"w":$elm$core$Maybe$Nothing,"x":A2($elm$core$List$map,(function($arg240_0){var $l25=$arg240_0;return _List_fromArray([($elm$html$Html$text)($l25)]);}),($l24)["L"]),"a":'Source Directories'}));}}{var $l26=$tailCase280.a;return ($author$project$Index$Skeleton$box)(({"w":$elm$core$Maybe$Nothing,"x":_List_fromArray([_List_fromArray([($elm$html$Html$text)(('Name: '+($elm$project_metadata_utils$Elm$Package$toString)(($l26)["m"])))]),_List_fromArray([($elm$html$Html$text)(('Version: '+($elm$project_metadata_utils$Elm$Version$toString)(($l26)["as"])))]),_List_fromArray([($elm$html$Html$text)(('License: '+($elm$project_metadata_utils$Elm$License$toString)(($l26)["aj"])))])]),"a":'Package Info'}));};});
var $author$project$Index$viewRightColumn=F2(function($l14,$l15){return A2($elm$html$Html$section,_List_fromArray([($elm$html$Html$Attributes$class)('right-column')]),(function($case158){if($case158.$===1){{return _List_Nil;}}{var $l16=$case158.a;return _List_fromArray([($author$project$Index$viewProjectSummary)($l16),A2($author$project$Index$viewDeps,$l14,$l16),A2($author$project$Index$viewTestDeps,$l14,$l16)]);}})($l15));;});
var $author$project$Index$viewFile=(function($tailInput0){var $l22=$tailInput0["m"];if(A2($elm$core$String$startsWith,'.',$l22)){return $elm$core$Maybe$Nothing;}else{return ($elm$core$Maybe$Just)(_List_fromArray([A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)($l22)]),_List_fromArray([($author$project$Index$Icon$lookup)($l22),($elm$html$Html$text)($l22)]))]));};});
var $author$project$Index$viewDir=(function($l21){if((A2($elm$core$String$startsWith,'.',$l21)||_Utils_eq($l21,'elm-stuff'))){return $elm$core$Maybe$Nothing;}else{return ($elm$core$Maybe$Just)(_List_fromArray([A2($elm$html$Html$a,_List_fromArray([($elm$html$Html$Attributes$href)($l21)]),_List_fromArray([$author$project$Index$Icon$folder,($elm$html$Html$text)($l21)]))]));};});
var $author$project$Index$viewFiles=F2(function($l19,$l20){return ($author$project$Index$Skeleton$box)(({"w":$elm$core$Maybe$Nothing,"x":_Utils_ap(A2($elm$core$List$filterMap,$author$project$Index$viewDir,($elm$core$List$sort)($l19)),A2($elm$core$List$filterMap,$author$project$Index$viewFile,A2($elm$core$List$sortBy,(function($record){return $record["m"];}),$l20))),"a":'File Navigation'}));;});
var $author$project$Index$viewReadme=(function($l17){let $tailCase167=$l17;if($tailCase167.$===1){{return ($elm$html$Html$text)('');}}{var $l18=$tailCase167.a;return ($author$project$Index$Skeleton$readmeBox)($l18);};});
var $author$project$Index$viewLeftColumn=F3(function($l11,$l12,$l13){return A2($elm$html$Html$section,_List_fromArray([($elm$html$Html$Attributes$class)('left-column')]),_List_fromArray([A2($author$project$Index$viewFiles,$l11,$l12),($author$project$Index$viewReadme)($l13)]));;});
var $author$project$Index$view=(function($l2){let $tailCase123=$l2;if($tailCase123.$===1){{var $l3=$tailCase123.a;return ({"v":_List_fromArray([($elm$html$Html$text)(($elm$json$Json$Decode$errorToString)($l3))]),"a":'???'});}}{var $l4=$tailCase123.a["aI"];var $l5=$tailCase123.a["aF"];var $l6=$tailCase123.a["L"];var $l7=$tailCase123.a["aA"];var $l8=$tailCase123.a["aH"];var $l9=$tailCase123.a["aE"];var $l10=$tailCase123.a["ay"];return ({"v":_List_fromArray([A2($elm$html$Html$header,_List_fromArray([($elm$html$Html$Attributes$class)('header')]),_List_Nil),A2($elm$html$Html$div,_List_fromArray([($elm$html$Html$Attributes$class)('content')]),_List_fromArray([A2($author$project$Index$Navigator$view,$l4,$l5),A3($author$project$Index$viewLeftColumn,$l6,$l7,$l8),A2($author$project$Index$viewRightColumn,$l10,$l9),A2($elm$html$Html$div,_List_fromArray([A2($elm$html$Html$Attributes$style,'clear','both')]),_List_Nil)]))]),"a":A2($elm$core$String$join,'/',A2($elm$core$List$cons,'~',$l5))});};});
var $author$project$Index$File=(function($r0){return (function($r1){return ({"m":$r0,"aP":$r1});});});
var $author$project$Index$fileDecoder=A3($elm$json$Json$Decode$map2,$author$project$Index$File,A2($elm$json$Json$Decode$field,'name',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'runnable',$elm$json$Json$Decode$bool));
var $author$project$Index$Flags=(function($r0){return (function($r1){return (function($r2){return (function($r3){return (function($r4){return (function($r5){return (function($r6){return ({"L":$r2,"ay":$r6,"aA":$r3,"aE":$r5,"aF":$r1,"aH":$r4,"aI":$r0});});});});});});});});
var $author$project$Index$decoder=A8($elm$json$Json$Decode$map7,$author$project$Index$Flags,A2($elm$json$Json$Decode$field,'root',$elm$json$Json$Decode$string),A2($elm$json$Json$Decode$field,'pwd',($elm$json$Json$Decode$list)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'dirs',($elm$json$Json$Decode$list)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'files',($elm$json$Json$Decode$list)($author$project$Index$fileDecoder)),A2($elm$json$Json$Decode$field,'readme',($elm$json$Json$Decode$nullable)($elm$json$Json$Decode$string)),A2($elm$json$Json$Decode$field,'outline',($elm$json$Json$Decode$nullable)($elm$project_metadata_utils$Elm$Project$decoder)),A2($elm$json$Json$Decode$field,'exactDeps',($elm$json$Json$Decode$dict)($elm$project_metadata_utils$Elm$Version$decoder)));
var $author$project$Index$main=($elm$browser$Browser$document)(({"f":(function($arg7_0){var $l1=$arg7_0;return _Utils_Tuple2(A2($elm$json$Json$Decode$decodeValue,$author$project$Index$decoder,$l1),$elm$core$Platform$Cmd$none);}),"j":(function($arg13_0){return $elm$core$Platform$Sub$none;}),"h":F2(function($arg11_0,$arg11_1){var $l0=$arg11_1;return _Utils_Tuple2($l0,$elm$core$Platform$Cmd$none);}),"k":$author$project$Index$view}));
_Platform_effectManagers["Task"]=_Platform_createManager($elm$core$Task$init,$elm$core$Task$onEffects,$elm$core$Task$onSelfMsg,$elm$core$Task$cmdMap,0);
_Platform_export({"NotFound":{'init':$author$project$NotFound$main(_Json_succeed(_Utils_Tuple0))(0)}});
_Platform_export({"Errors":{'init':$author$project$Errors$main($elm$json$Json$Decode$value)(0)}});
_Platform_export({"Index":{'init':$author$project$Index$main($elm$json$Json$Decode$value)(0)}});

}(this));
