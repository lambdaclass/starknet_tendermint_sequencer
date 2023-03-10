%builtins output

func main(output_ptr: felt*) -> (output_ptr: felt*) {
    // Call fib(1, 1, 10).
    let result: felt = fib(1, 1, 500);
    assert [output_ptr] = result;
    return (output_ptr=output_ptr+1);
}

func fib(first_element, second_element, n) -> (res: felt) {
    jmp fib_body if n != 0;
    tempvar result = second_element;
    return (second_element,);

    fib_body:
    tempvar y = first_element + second_element;
    return fib(second_element, y, n - 1);
}
