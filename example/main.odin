package main

import "core:fmt"
import "core:math"

Person :: struct {
	name:   string,
	age:    int,
	height: f32,
}

fibonacci :: proc(n: int) -> int {
	if n <= 1 {
		return n
	}
	return fibonacci(n - 1) + fibonacci(n - 2)
}

calculate_area :: proc(radius: f32) -> f32 {
	return math.PI * radius * radius
}

main :: proc() {
	fmt.println("Odin Debugging Example")
	fmt.println("=====================")

	person := Person {
		name   = "Alice",
		age    = 30,
		height = 5.6,
	}

	fmt.printf("Person: %v\n", person)

	// Calculate some fibonacci numbers
	fmt.println("\nFibonacci sequence:")
	for i in 0 ..< 10 {
		fib := fibonacci(i)
		fmt.printf("fib(%d) = %d\n", i, fib)
	}

	fmt.println("\nCircle areas:")
	radii := []f32{1.0, 2.5, 5.0, 10.0}

	for radius in radii {
		area := calculate_area(radius)
		fmt.printf("Circle with radius %.1f has area %.2f\n", radius, area)
	}

	numbers := []int{1, 2, 3, 4, 5}
	sum := 0

	for num in numbers {
		sum += num
	}

	fmt.printf("\nSum of numbers: %d\n", sum)

	ptr := &person
	fmt.printf("Person via pointer: %v\n", ptr^)

	fmt.println("\nProgram completed successfully!")
}
